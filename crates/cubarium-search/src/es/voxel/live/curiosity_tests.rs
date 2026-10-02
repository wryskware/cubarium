//! Brain curiosity (`design/handoffs/brain-curiosity-2026-10-02.md`): the novelty term in
//! the window score. Written from the brief's eight tests before the implementation; the
//! accounting is driven directly with positions, no sim runs.

use cubarium_voxel_fauna::{Founder, FounderPhysiology};

use super::curiosity::{Curiosity, Geometry, LineageCuriosity, Memory};
use super::window::{NetEnergy, TICKS_PER_MIN};

/// A ten-minute window, ticks.
const WINDOW: u64 = 10 * TICKS_PER_MIN;

fn lineage(weight: f64) -> LineageCuriosity {
    LineageCuriosity {
        weight,
        memory_min: 30.0,
        cell_bl: 1.0,
    }
}

fn geometry(f: Founder, c: &LineageCuriosity) -> Geometry {
    Geometry::of(&FounderPhysiology::frozen(f), c)
}

fn adult_m(f: Founder) -> f64 {
    let p = FounderPhysiology::frozen(f);
    p.body_at(p.core.body_max).length_m
}

/// The centre of cell `k` along x (row z = 0) for a lineage's cells.
fn centre(g: &Geometry, k: i64) -> (f64, f64) {
    ((k as f64 + 0.5) * g.cell_m, 0.5 * g.cell_m)
}

/// Walk `id` through `cells` (cell indices along x), holding each for `hold` ticks;
/// returns what it earned. The first observation of a fresh id is its spawn, not an entry.
fn walk(m: &mut Memory, id: u64, g: &Geometry, cells: &[i64], hold: u64) -> f64 {
    let mut earned = 0.0;
    for &k in cells {
        for _ in 0..hold {
            let (x, z) = centre(g, k);
            earned += m.observe(id, g, x, z);
            m.advance();
        }
    }
    earned
}

/// Test 1: With weight 0 the score is today's `NetEnergy` score, compared with `==`.
#[test]
fn zero_weight_is_todays_score() {
    for (e0, e1, given, died) in [
        (1.0, 1.5, 0.25, false),
        (2.0, 0.7, 0.0, false),
        (1.3, 1.9, 0.4, true),
    ] {
        let today = NetEnergy::of(e0, e1, given, died, 0.3, 0.1, 0.5);
        for novelty in [0.0, 0.37, 1.0, 2.5] {
            let s = today.with_curiosity(novelty, 0.0);
            assert!(s.score == today.score, "{} vs {}", s.score, today.score);
            assert!(s.delta_normalized == today.delta_normalized);
            assert!(s.given_normalized == today.given_normalized);
            assert_eq!(s.novelty, novelty);
            assert!(s.curiosity == 0.0);
        }
    }
    // Without a file every weight is 0.
    let none = Curiosity::default();
    for f in Founder::ALL {
        assert!(none.of(f).weight == 0.0, "{f:?}");
    }
}

/// Test 2: Entering N fresh cells in one window earns novelty `N / n_cruise`, and `n_cruise`
/// is the window's seconds × the lineage's cruise ÷ the cell's metres.
#[test]
fn a_straight_line_earns_one_per_fresh_cell() {
    for f in [Founder::Blind, Founder::Browser] {
        let c = lineage(1.0);
        let g = geometry(f, &c);
        let phys = FounderPhysiology::frozen(f);
        let adult = adult_m(f);
        assert!((g.cell_m - c.cell_bl * adult).abs() < 1e-12);
        assert!((g.cruise_m_per_s - phys.cruise_body_lengths_per_s * adult).abs() < 1e-12);
        let n_cruise = WINDOW as f64 * cubarium_voxel::DT * g.cruise_m_per_s / g.cell_m;
        assert!((g.n_cruise(WINDOW) - n_cruise).abs() < 1e-9);
        for n in [1usize, 5, 40] {
            let mut m = Memory::default();
            // Spawn in cell 0, then enter cells 1..=n.
            let cells: Vec<i64> = (0..=n as i64).collect();
            let earned = walk(&mut m, 7, &g, &cells, 3);
            assert!((earned - n as f64).abs() < 1e-12, "{f:?} n={n}: {earned}");
            let novelty = g.novelty(earned, WINDOW);
            assert!(
                (novelty - n as f64 / n_cruise).abs() < 1e-12,
                "{f:?}: {novelty}"
            );
        }
    }
}

/// Test 3: Rocking between two cells for a whole window earns at most 2 + ε, ε the decay
/// over the window; spinning in one cell earns 0.
#[test]
fn rocking_earns_about_two_cells_and_spinning_nothing() {
    let c = lineage(1.0);
    let g = geometry(Founder::Browser, &c);
    let half_life = c.memory_min * TICKS_PER_MIN as f64;
    let eps = WINDOW as f64 * std::f64::consts::LN_2 / half_life;
    for period in [1u64, 7, 600] {
        let mut m = Memory::default();
        let mut earned = 0.0;
        for t in 0..WINDOW {
            let k = ((t / period) % 2) as i64;
            let (x, z) = centre(&g, k);
            earned += m.observe(1, &g, x, z);
            m.advance();
        }
        assert!(earned <= 2.0 + eps, "period {period}: {earned} > 2 + {eps}");
        assert!(
            earned >= 1.0 - 1e-12,
            "period {period}: the second cell is fresh"
        );
    }
    // Spinning in place: positions wander inside one cell.
    let mut m = Memory::default();
    let mut earned = 0.0;
    for t in 0..WINDOW {
        let a = t as f64 * 0.3;
        let (x, z) = centre(&g, 3);
        earned += m.observe(
            2,
            &g,
            x + 0.3 * g.cell_m * a.cos(),
            z + 0.3 * g.cell_m * a.sin(),
        );
        m.advance();
    }
    assert!(earned == 0.0, "spinning earned {earned}");
}

/// Test 4: A return to a cell exactly one `memory_min` after leaving it earns 0.5.
#[test]
fn a_return_after_one_half_life_earns_half() {
    for memory_min in [30.0, 7.5] {
        let c = LineageCuriosity {
            weight: 1.0,
            memory_min,
            cell_bl: 1.0,
        };
        let g = geometry(Founder::Blind, &c);
        let h = (memory_min * TICKS_PER_MIN as f64).round() as u64;
        let mut m = Memory::default();
        let (ax, az) = centre(&g, 0);
        let (bx, bz) = centre(&g, 1);
        assert_eq!(m.observe(3, &g, ax, az), 0.0, "spawn");
        m.advance();
        assert!(
            (m.observe(3, &g, bx, bz) - 1.0).abs() < 1e-12,
            "first visit"
        );
        // It left A on this tick; stay in B until one half-life has passed.
        for i in 0..h {
            m.advance();
            if i + 1 < h {
                assert_eq!(m.observe(3, &g, bx, bz), 0.0, "staying earns nothing");
            }
        }
        let back = m.observe(3, &g, ax, az);
        assert!((back - 0.5).abs() < 1e-6, "memory_min {memory_min}: {back}");
    }
}

/// Test 5: A body keeps its familiarity across a window boundary, so its second window over
/// the same ground earns less than its first; a newborn starts empty; a dead body's id
/// holds no memory.
#[test]
fn familiarity_lives_across_windows_and_dies_with_the_body() {
    let c = lineage(1.0);
    let g = geometry(Founder::Browser, &c);
    let ground: Vec<i64> = (0..30).collect();
    let mut m = Memory::default();
    let hold = WINDOW / ground.len() as u64;
    let first = walk(&mut m, 11, &g, &ground, hold);
    // The window boundary: the memory is carried, nothing is reset.
    let second = walk(&mut m, 11, &g, &ground, hold);
    assert!(first > 28.0, "{first}");
    assert!(second < first, "second window {second} vs first {first}");
    assert!(
        second < 0.5 * first,
        "the ground is still familiar: {second}"
    );
    // A newborn on the same ground starts empty.
    assert!(!m.knows(12));
    let newborn = walk(&mut m, 12, &g, &ground, hold);
    assert!((newborn - first).abs() < 1e-12, "{newborn} vs {first}");
    // After it dies its id holds nothing.
    assert!(m.knows(11));
    m.forget(11);
    assert!(!m.knows(11));
    // And the memory is kept only for the living.
    m.retain(|id| id != 12);
    assert!(!m.knows(12));
}

/// Test 6: A frondgrazer and a littershredder on identical tracks and energies: their scores
/// differ by `w_frond·novelty_frond − w_shred·novelty_shred`, each novelty on its own
/// lineage's adult length and cruise.
#[test]
fn each_lineage_scores_on_its_own_cells_and_weight() {
    let mut cur = Curiosity::default();
    cur.lineages[Founder::Browser.index()] = lineage(0.8);
    cur.lineages[Founder::Blind.index()] = lineage(0.3);
    let (gf, gs) = (
        geometry(Founder::Browser, cur.of(Founder::Browser)),
        geometry(Founder::Blind, cur.of(Founder::Blind)),
    );
    assert!(
        (adult_m(Founder::Browser) - adult_m(Founder::Blind)).abs() > 1e-6,
        "the test needs two adult lengths"
    );
    // One straight 6 m track along x, sampled every tick.
    let track: Vec<(f64, f64)> = (0..WINDOW)
        .map(|t| (0.05 + 6.0 * t as f64 / WINDOW as f64, 0.1))
        .collect();
    let earn = |g: &Geometry| {
        let mut m = Memory::default();
        let mut e = 0.0;
        for &(x, z) in &track {
            e += m.observe(5, g, x, z);
            m.advance();
        }
        e
    };
    let (ef, es) = (earn(&gf), earn(&gs));
    let (nf, ns) = (gf.novelty(ef, WINDOW), gs.novelty(es, WINDOW));
    // Each novelty is its own cells over its own cruise.
    for (f, g, e, n) in [
        (Founder::Browser, &gf, ef, nf),
        (Founder::Blind, &gs, es, ns),
    ] {
        let cell = adult_m(f);
        assert!(
            (e - (6.0 / cell).floor()).abs() <= 1.0,
            "{f:?}: {e} cells of {cell} m"
        );
        let cruise = FounderPhysiology::frozen(f).cruise_body_lengths_per_s * cell;
        let n_cruise = WINDOW as f64 * cubarium_voxel::DT * cruise / cell;
        assert!((n - e / n_cruise).abs() < 1e-12, "{f:?}");
        assert!((g.cell_m - cell).abs() < 1e-12);
    }
    assert!(nf != ns);
    let energy = NetEnergy::of(1.0, 1.4, 0.2, false, 0.5, 0.1, 0.5);
    let sf = energy.with_curiosity(nf, cur.of(Founder::Browser).weight);
    let ss = energy.with_curiosity(ns, cur.of(Founder::Blind).weight);
    let expect = 0.8 * nf - 0.3 * ns;
    assert!(sf.curiosity == 0.8 * nf && ss.curiosity == 0.3 * ns);
    assert!(sf.score == energy.score + sf.curiosity && ss.score == energy.score + ss.curiosity);
    assert!(
        ((sf.score - ss.score) - expect).abs() <= 1e-12,
        "{} vs {expect}",
        sf.score - ss.score
    );
}

/// Test 7: The file refuses an unknown lineage, a negative weight, `memory_min = 0`,
/// `cell_bl = 0` and an unknown field; a good file loads.
#[test]
fn the_config_refuses_bad_files() {
    let good = "[frondgrazer]\nweight = 1.5\nmemory_min = 30\ncell_bl = 1.0\n\
                [littershredder]\nweight = 0.2\nmemory_min = 30\ncell_bl = 1.0\n";
    let c = Curiosity::parse(good).expect("good file");
    assert_eq!(c.of(Founder::Browser).weight, 1.5);
    assert_eq!(c.of(Founder::Blind).weight, 0.2);
    assert_eq!(c.of(Founder::Lanternjaw).weight, 0.0, "absent: weight 0");
    for (bad, why) in [
        (
            "[seedporter]\nweight = 1\nmemory_min = 30\ncell_bl = 1\n",
            "seedporter",
        ),
        (
            "[frondgrazer]\nweight = -0.1\nmemory_min = 30\ncell_bl = 1\n",
            "weight",
        ),
        (
            "[frondgrazer]\nweight = 1\nmemory_min = 0\ncell_bl = 1\n",
            "memory_min",
        ),
        (
            "[frondgrazer]\nweight = 1\nmemory_min = 30\ncell_bl = 0\n",
            "cell_bl",
        ),
        (
            "[frondgrazer]\nweight = 1\nmemory_min = 30\ncell_bl = 1\nshare = 0.5\n",
            "share",
        ),
    ] {
        let err = Curiosity::parse(bad).expect_err(why);
        assert!(err.contains(why), "{why}: {err}");
    }
}

/// Test 8: Novelty earned before a death is kept in the score, and `delta` uses `E_end = 0`.
#[test]
fn novelty_before_a_death_is_kept() {
    let c = lineage(0.7);
    let g = geometry(Founder::Browser, &c);
    let mut m = Memory::default();
    let earned = walk(&mut m, 21, &g, &[0, 1, 2, 3, 4], 20);
    assert!((earned - 4.0).abs() < 1e-12);
    m.forget(21);
    assert!(!m.knows(21));
    let novelty = g.novelty(earned, WINDOW);
    let (e0, e_end, given, r) = (1.2, 1.6, 0.1, 0.5);
    let dead = NetEnergy::of(e0, e_end, given, true, 0.0, 0.0, r).with_curiosity(novelty, c.weight);
    assert!(dead.died);
    assert!((dead.delta_normalized - (0.0 - e0) / r).abs() < 1e-12);
    assert!(dead.novelty == novelty && novelty > 0.0);
    let expect = (0.0 - e0) / r + given / r + c.weight * novelty;
    assert!(
        (dead.score - expect).abs() < 1e-12,
        "{} vs {expect}",
        dead.score
    );
}

/// Wiring (beyond the brief's eight): a window hands on a memory of exactly its living
/// bodies, and counts every lineage's novelty. 2 × 100 ticks of a training arena.
#[test]
fn a_window_hands_its_living_bodies_memory_on() {
    use super::window::Window;
    use crate::es::voxel::driver::sampled_stores;
    use crate::es::voxel::task::{PreparedArena, Stage, TRAINING_LAYOUT_SEEDS};
    use cubarium_voxel_sim::{ScheduleMode, SimConfig};

    let founder = Founder::Browser;
    let seed = TRAINING_LAYOUT_SEEDS[0];
    let prepared = PreparedArena::build_stage(founder, seed, Stage::A);
    let mut arena = prepared.episode_arena();
    arena.refound(sampled_stores(founder, seed, seed, 0));
    let mut sim = arena.into_sim_prepared(SimConfig::with_threads(1), prepared.episode_senses());
    sim.set_mode(ScheduleMode::Static);
    let mut cur = Curiosity::default();
    cur.lineages[founder.index()] = lineage(1.0);
    let tables = vec![None; Founder::COUNT];
    let mut memory = Memory::default();
    for _ in 0..2 {
        let mut w = Window::begin_with(&mut sim, &tables, &[0; Founder::COUNT], 100, &cur, memory);
        while !w.done() {
            w.step(&mut sim);
        }
        let mut r = w.finish(&sim, 0);
        memory = std::mem::take(&mut r.memory);
        let living: Vec<u64> = sim
            .fauna()
            .view()
            .animals
            .iter()
            .filter(|a| a.founder.is_some())
            .map(|a| a.id)
            .collect();
        assert!(!living.is_empty());
        assert_eq!(memory.len(), living.len());
        assert!(living.iter().all(|id| memory.knows(*id)));
        let s = &r.lineages[founder.index()];
        assert!(s.novelty_bodies > 0);
        assert!(s.mean_novelty().is_finite() && s.mean_novelty() >= 0.0);
        assert!((s.curiosity_sum - s.novelty_sum).abs() < 1e-12, "weight 1");
    }
}

/// The shipped file loads, and the ambusher's weight is 0.
#[test]
fn the_shipped_curiosity_file_loads() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/training/curiosity.toml");
    let c = Curiosity::load(&path).expect("shipped file");
    assert_eq!(c.of(Founder::Lanternjaw).weight, 0.0);
    for f in Founder::ALL {
        assert_eq!(c.of(f).memory_min, 30.0);
        assert_eq!(c.of(f).cell_bl, 1.0);
    }
}
