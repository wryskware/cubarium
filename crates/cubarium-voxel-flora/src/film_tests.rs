//! Glassfilm (`design/handoffs/voxel-roster-2026-10-01.md`, brief R2a): a fourth ground
//! pool, a living film on shallow submerged faces that grows from light on the site's
//! mineral, spreads to wetter-than-empty neighbours, dies to litter when its site dries or
//! deepens, and can be grazed with [`Flora::take_film`].
//!
//! Fixtures: a one-deep strip of bedrock floor (`y = 0`) with whole cells of water placed
//! over chosen columns, the day clock off, the plant layer provisioned at creation so every
//! face has a `Ground`. **The world is never stepped**: the water is placed and holds still,
//! and the film's clock is the plant layer's own ([`Flora::tick`]), as the seed bank's is.
//! A "dried" or "deepened" site is the same terrain with other water, a twin world the
//! flora is stepped against. No run is longer than 200 ticks (10 s).

use crate::snapshot::{SCHEMA, decode, encode};
use crate::{Flora, FloraConfig, Provision, Site};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, TICK_HZ, World};

const VOXEL_M: f64 = 0.25;
const AREA_M2: f64 = VOXEL_M * VOXEL_M;
const TICKS: u32 = 200;

/// The face of column `x` of a strip.
fn face(x: u32) -> Site {
    Site { x, y: 0, z: 0 }
}

/// A one-deep strip, `cells.len()` columns wide, bedrock floor at `y = 0`, with
/// `cells[x]` whole cells of water standing over column `x` (0.25 m each). Day clock off,
/// so the light is the sky's alone.
fn strip(cells: &[u32]) -> World {
    let mut w = World::empty(VoxelConfig {
        width: cells.len() as u32,
        height: 6,
        depth: 1,
        voxel_m: VOXEL_M,
        seed: 17,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        ..VoxelConfig::default()
    });
    let cell = w.config().voxel_volume();
    for (x, &n) in cells.iter().enumerate() {
        for y in 1..=n {
            w.apply(WorldCommand::AddWater {
                x: x as i64,
                y,
                z: 0,
                volume_m3: cell,
            });
        }
    }
    for (x, &n) in cells.iter().enumerate() {
        let d = w.view().standing_depth_m(x as i64, 0, 0);
        assert!(
            (d - f64::from(n) * VOXEL_M).abs() < 1e-12,
            "fixture: column {x} stands {d} m, want {} cells",
            n
        );
        assert_eq!(
            w.view().sky_visibility(x as i64, 0, 0),
            1.0,
            "fixture: open sky"
        );
    }
    w
}

/// A sealed bedrock box with a cavity at `z = 1`, `x = 1..=3`, `y = 1..=2`, one cell of
/// water over each cavity face `(x, 0, 1)`: eligible water, no sky.
fn cave() -> (World, Vec<Site>) {
    let (width, height, depth) = (5u32, 4u32, 3u32);
    let mut w = World::empty(VoxelConfig {
        width,
        height,
        depth,
        voxel_m: VOXEL_M,
        seed: 19,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        ..VoxelConfig::default()
    });
    for z in 0..depth {
        for y in 1..height {
            for x in 0..i64::from(width) {
                let cavity = z == 1 && (1..=2).contains(&y) && (1..=3).contains(&x);
                if !cavity {
                    w.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Bedrock,
                    });
                }
            }
        }
    }
    let cell = w.config().voxel_volume();
    let mut sites = Vec::new();
    for x in 1..=3u32 {
        w.apply(WorldCommand::AddWater {
            x: i64::from(x),
            y: 1,
            z: 1,
            volume_m3: cell,
        });
        let site = Site { x, y: 0, z: 1 };
        assert!((w.view().standing_depth_m(i64::from(x), 0, 1) - VOXEL_M).abs() < 1e-12);
        assert_eq!(
            w.view().sky_visibility(i64::from(x), 0, 1),
            0.0,
            "fixture: no sky"
        );
        sites.push(site);
    }
    (w, sites)
}

/// The shipped config, every face provisioned at creation.
fn config() -> FloraConfig {
    FloraConfig {
        provision: Provision::AtCreation,
        ..FloraConfig::default()
    }
}

fn run(flora: &mut Flora, world: &mut World, ticks: u32) {
    for _ in 0..ticks {
        flora.step(world);
    }
}

/// Put `organic` of film on `site` directly (mineral 0.02, energy 2.0 per unit), booked as
/// introduced exactly as [`Flora::seed_film`] books it. The site must have a `Ground`.
fn put_film(flora: &mut Flora, site: Site, organic: f64) {
    let i = flora
        .ground
        .binary_search_by_key(&site, |g| g.site)
        .expect("a provisioned site");
    let g = &mut flora.ground[i];
    g.film += organic;
    g.film_mineral += 0.02 * organic;
    g.film_energy += 2.0 * organic;
    flora.ledger.seeded_organic_in += organic;
    flora.ledger.seeded_mineral_in += 0.02 * organic;
    flora.ledger.seeded_energy_in += 2.0 * organic;
    flora.ledger.film_seeded += organic;
}

fn ground(flora: &Flora, site: Site) -> crate::Ground {
    flora
        .view()
        .ground_at(site)
        .expect("a provisioned site")
        .clone()
}

/// Film, its mineral and its energy summed over every site.
fn film_totals(flora: &Flora) -> (f64, f64, f64) {
    flora.ground.iter().fold((0.0, 0.0, 0.0), |(o, m, e), g| {
        (o + g.film, m + g.film_mineral, e + g.film_energy)
    })
}

/// Every mineral stock on the ground: the site pools, litter's, dead wood's, carrion's and
/// the film's.
fn ground_mineral(flora: &Flora) -> f64 {
    flora
        .ground
        .iter()
        .map(|g| {
            g.mineral + g.litter_mineral + g.dead_wood_mineral + g.carrion_mineral + g.film_mineral
        })
        .sum()
}

fn assert_closes(flora: &Flora, when: &str) {
    let v = flora.view();
    for (held, expected, what) in [
        (v.organic(), v.ledger.expected_organic(), "organic"),
        (v.mineral(), v.ledger.expected_mineral(), "mineral"),
        (v.energy(), v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            (held - expected).abs() <= 1e-12 * held.abs().max(1e-6),
            "{when}: flora {what} holds {held}, the ledger expects {expected}"
        );
    }
}

/// The film's own budget: what stands as film is what was introduced and grown, less what
/// was respired, died to litter and was eaten. Spread books nothing.
fn assert_film_budget(flora: &Flora, when: &str) {
    let l = &flora.view().ledger;
    let held = film_totals(flora).0;
    let want =
        l.film_seeded + l.film_produced - l.film_respired - l.film_to_litter - l.film_consumed;
    assert!(
        (held - want).abs() <= 1e-12 * held.abs().max(l.film_seeded).max(1e-9),
        "{when}: {held} film stands, the film terms give {want} ({l:?})"
    );
}

/// The brief's growth law at film `f` on a site `depth_m` under water, open sky:
/// `r · exp(−k · d) · (1 − f / K) − m`, per second, per unit of film.
fn growth_rate(c: &FloraConfig, depth_m: f64, f: f64) -> f64 {
    let light = (-c.film_extinction_per_m * depth_m).exp();
    let capacity = c.film_capacity_per_m2 * AREA_M2;
    c.film_growth_per_s * light * (1.0 - f / capacity) - c.film_maintenance_per_s
}

// ------------------------------------------------------------------------ growth

/// Founding puts `film_seed_per_m2` on every wet-eligible face and nothing on a dry one or
/// one deeper than the maximum; a lit shallow film then grows by the brief's logistic law,
/// slower under more water, drawing its mineral from the site's pool; the readers see it.
#[test]
fn film_grows_on_a_lit_shallow_submerged_site() {
    // Column 1 stands 0.25 m, column 3 0.5 m, column 5 0.75 m (past 0.6), the rest dry.
    // Dry columns between them: no two eligible sites touch, so nothing spreads.
    let mut world = strip(&[0, 1, 0, 2, 0, 3, 0, 0]);
    let mut flora = Flora::in_world(&world, config());
    let c = flora.config().clone();
    let seeded = flora.seed_film(&world);
    assert_eq!(seeded, 2, "two eligible faces");
    let f0 = c.film_seed_per_m2 * AREA_M2;
    for x in 0..8 {
        let want = if x == 1 || x == 3 { f0 } else { 0.0 };
        let got = ground(&flora, face(x)).film;
        assert!(
            (got - want).abs() <= 1e-15 * f0,
            "column {x}: seeded {got}, want {want}"
        );
    }
    let g0 = ground(&flora, face(1));
    assert!(
        g0.film_mineral > 0.0 && g0.film_energy > 0.0,
        "a seeded film carries mineral and energy"
    );
    assert_closes(&flora, "seeded");
    let site_mineral0 = g0.mineral + g0.litter_mineral + g0.film_mineral;

    run(&mut flora, &mut world, TICKS);
    let t = f64::from(TICKS) * DT;

    let mut observed = [0.0; 2];
    for (k, (x, depth_m)) in [(1, 0.25), (3, 0.5)].into_iter().enumerate() {
        let f = ground(&flora, face(x)).film;
        assert!(f > f0, "column {x}: film {f} did not grow from {f0}");
        let rate = (f / f0).ln() / t;
        let want = growth_rate(&c, depth_m, f0);
        assert!(want > 0.0);
        assert!(
            (rate - want).abs() <= 0.15 * want,
            "column {x} at {depth_m} m grew at {rate} /s, the law gives {want} /s"
        );
        observed[k] = rate;
    }
    assert!(
        observed[1] < observed[0],
        "deeper water, less light, slower growth"
    );
    for x in [0, 2, 4, 5, 6, 7] {
        assert_eq!(ground(&flora, face(x)).film, 0.0, "column {x} holds film");
    }

    // The nutrient route: the site's pool paid for the new tissue, and the site's mineral
    // went nowhere else.
    let g = ground(&flora, face(1));
    assert!(g.mineral < g0.mineral, "growth drew no mineral");
    assert!(g.film_mineral > g0.film_mineral);
    let site_mineral = g.mineral + g.litter_mineral + g.film_mineral;
    assert!((site_mineral - site_mineral0).abs() <= 1e-12 * site_mineral0);

    let l = &flora.view().ledger;
    assert!(l.film_produced > 0.0 && l.fixed_in > 0.0 && l.light_in > 0.0);
    assert_closes(&flora, "grown");
    assert_film_budget(&flora, "grown");

    // Readers.
    let v = flora.view();
    assert_eq!(v.film_at(face(1)), g.film);
    assert_eq!(v.film_at(face(0)), 0.0);
    let sites: Vec<Site> = v.film_sites().map(|(s, _)| s).collect();
    assert_eq!(sites, vec![face(1), face(3)]);
}

// ----------------------------------------------------------------- no growth when

/// Seed on shallow water, then step against the same terrain with other water.
fn seeded_then_stepped_against(cells: &[u32], config: FloraConfig) -> (Flora, f64) {
    let wet = strip(&vec![1; cells.len()]);
    let mut flora = Flora::in_world(&wet, config);
    assert_eq!(flora.seed_film(&wet), cells.len());
    let f0 = flora.config().film_seed_per_m2 * AREA_M2;
    let mut other = strip(cells);
    run(&mut flora, &mut other, TICKS);
    (flora, f0)
}

fn assert_no_growth(flora: &Flora, sites: &[Site], f0: f64, when: &str) {
    assert_eq!(flora.view().ledger.film_produced, 0.0, "{when}: film grew");
    for &s in sites {
        let f = ground(flora, s).film;
        assert!(
            f < f0,
            "{when}: {s:?} holds {f}, seeded {f0}: it did not decline"
        );
    }
    assert_closes(flora, when);
    assert_film_budget(flora, when);
}

/// Depth 0: a seeded film whose water is gone grows nothing and declines.
#[test]
fn film_does_not_grow_on_a_dry_site() {
    let (flora, f0) = seeded_then_stepped_against(&[0; 6], config());
    let sites: Vec<Site> = (0..6).map(face).collect();
    assert_no_growth(&flora, &sites, f0, "dry");
}

/// Past `film_max_depth_m` (0.75 m against 0.6): lit, but not eligible, so no growth.
#[test]
fn film_does_not_grow_deeper_than_the_maximum() {
    let (flora, f0) = seeded_then_stepped_against(&[3; 6], config());
    let sites: Vec<Site> = (0..6).map(face).collect();
    assert_no_growth(&flora, &sites, f0, "deep");
}

/// Eligible water with no sky: no light, no growth; upkeep alone.
#[test]
fn film_does_not_grow_in_the_dark() {
    let (mut world, sites) = cave();
    let mut flora = Flora::in_world(&world, config());
    assert_eq!(
        flora.seed_film(&world),
        sites.len(),
        "the cavity faces are eligible"
    );
    let f0 = flora.config().film_seed_per_m2 * AREA_M2;
    run(&mut flora, &mut world, TICKS);
    let l = &flora.view().ledger;
    assert_eq!(l.film_produced, 0.0, "dark: film grew");
    for &s in &sites {
        let f = ground(&flora, s).film;
        assert!(f > 0.0 && f <= f0, "dark: {s:?} holds {f}, seeded {f0}");
    }
    assert_closes(&flora, "dark");
    assert_film_budget(&flora, "dark");
}

/// No mineral in the site's pool, no new tissue: the film never stands above what it was
/// seeded with (it can at most rebuild what upkeep returns). The same strip with the
/// shipped mineral grows: the control.
#[test]
fn film_does_not_grow_without_mineral() {
    for (mineral, grows) in [(0.0, false), (FloraConfig::default().initial_mineral, true)] {
        let mut world = strip(&[1; 4]);
        let mut flora = Flora::in_world(
            &world,
            FloraConfig {
                initial_mineral: mineral,
                ..config()
            },
        );
        assert_eq!(flora.seed_film(&world), 4);
        let f0 = flora.config().film_seed_per_m2 * AREA_M2;
        run(&mut flora, &mut world, TICKS);
        for x in 0..4 {
            let f = ground(&flora, face(x)).film;
            if grows {
                assert!(f > f0, "control: column {x} did not grow ({f} from {f0})");
            } else {
                assert!(
                    f <= f0 * (1.0 + 1e-12),
                    "no mineral: column {x} grew to {f} from {f0}"
                );
            }
        }
        assert_closes(&flora, "mineral arm");
        assert_film_budget(&flora, "mineral arm");
    }
}

// ------------------------------------------------------------------------- death

/// A site that dries, or deepens past the maximum, loses film into its **own** litter at
/// about `film_dry_death_per_s`; mineral stays on the site. Decomposition is off so what
/// reached the litter is still there to count.
#[test]
fn drying_sends_film_to_litter() {
    for (cells, what) in [([0u32; 6], "dried"), ([3u32; 6], "deepened")] {
        let wet = strip(&[1; 6]);
        let mut flora = Flora::in_world(
            &wet,
            FloraConfig {
                decomposition: 0.0,
                ..config()
            },
        );
        assert_eq!(flora.seed_film(&wet), 6);
        let c = flora.config().clone();
        let before: Vec<crate::Ground> = (0..6).map(|x| ground(&flora, face(x))).collect();
        let (film0, _, _) = film_totals(&flora);
        let mineral0 = ground_mineral(&flora);
        let l0 = flora.view().ledger.clone();

        let mut world = strip(&cells);
        run(&mut flora, &mut world, TICKS);
        let t = f64::from(TICKS) * DT;
        let l = flora.view().ledger.clone();

        let to_litter = l.film_to_litter - l0.film_to_litter;
        let respired = l.film_respired - l0.film_respired;
        assert!(to_litter > 0.0, "{what}: no film went to litter");
        let k = c.film_dry_death_per_s;
        assert!(
            to_litter <= k * t * film0 * (1.0 + 1e-9) && to_litter >= 0.5 * k * t * film0,
            "{what}: {to_litter} to litter in {t} s from {film0} at {k} /s"
        );
        let mut litter_gain = 0.0;
        for (x, g0) in before.iter().enumerate() {
            let g = ground(&flora, face(x as u32));
            assert!(g.film < g0.film, "{what}: column {x} kept its film");
            assert!(
                g.litter > g0.litter,
                "{what}: column {x}'s own litter got nothing"
            );
            litter_gain += g.litter - g0.litter;
        }
        assert!(
            (litter_gain - to_litter).abs() <= 1e-12 * to_litter,
            "{what}: litter gained {litter_gain}, the ledger sent {to_litter}"
        );
        let (film, _, _) = film_totals(&flora);
        assert!(
            ((film0 - film) - (to_litter + respired)).abs() <= 1e-12 * film0,
            "{what}: film fell {} but {to_litter} went to litter and {respired} was respired",
            film0 - film
        );
        assert_eq!(l.film_produced, l0.film_produced, "{what}: film grew");
        assert!(
            (ground_mineral(&flora) - mineral0).abs() <= 1e-12 * mineral0,
            "{what}: mineral left the ground"
        );
        assert_closes(&flora, what);
        assert_film_budget(&flora, what);
    }
}

// ------------------------------------------------------------------------ spread

/// Film on one face of a pool reaches its empty eligible 4-neighbours and never a dry
/// face; the neighbour pays, so film, its mineral and its energy are conserved and move at
/// the source's own densities. Growth and upkeep are off so spread is all that moves.
#[test]
fn spread_moves_film_to_an_empty_eligible_neighbour_and_conserves_it() {
    let mut world = strip(&[0, 1, 1, 1, 1, 1, 0, 0]);
    let mut flora = Flora::in_world(
        &world,
        FloraConfig {
            film_growth_per_s: 0.0,
            film_maintenance_per_s: 0.0,
            ..config()
        },
    );
    let f0 = 0.01;
    put_film(&mut flora, face(3), f0);
    let (o0, m0, e0) = film_totals(&flora);
    assert_closes(&flora, "placed");

    run(&mut flora, &mut world, TICKS);
    let t = f64::from(TICKS) * DT;
    let k = flora.config().film_spread_per_s;

    let source = ground(&flora, face(3));
    assert!(source.film < f0, "the source paid nothing");
    let mut gained = 0.0;
    for x in [2, 4] {
        let g = ground(&flora, face(x));
        assert!(g.film > 0.0, "neighbour {x} got no film");
        assert!(
            (g.film_mineral / g.film - 0.02).abs() <= 1e-9
                && (g.film_energy / g.film - 2.0).abs() <= 1e-9,
            "neighbour {x}'s film is not the source's stuff"
        );
    }
    for x in [1, 2, 4, 5] {
        gained += ground(&flora, face(x)).film;
    }
    assert!(
        gained <= 2.0 * k * t * f0 * (1.0 + 1e-9),
        "{gained} moved in {t} s at {k} /s"
    );
    for x in [0, 6, 7] {
        assert_eq!(ground(&flora, face(x)).film, 0.0, "dry column {x} got film");
    }
    let (o, m, e) = film_totals(&flora);
    assert!((o - o0).abs() <= 1e-12 * o0, "film {o0} became {o}");
    assert!((m - m0).abs() <= 1e-12 * m0, "film mineral {m0} became {m}");
    assert!((e - e0).abs() <= 1e-12 * e0, "film energy {e0} became {e}");
    let l = &flora.view().ledger;
    assert_eq!(
        (l.film_produced, l.film_respired, l.film_to_litter),
        (0.0, 0.0, 0.0)
    );
    assert_closes(&flora, "spread");
    assert_film_budget(&flora, "spread");
}

// ---------------------------------------------------------------------- grazing

/// `take_film` is `take_litter`'s rule on the film: up to `want`, mineral and energy at the
/// pool's own density, an emptied pool keeps no dust, booked as consumed; `None` for
/// nothing to take or a bad `want`.
#[test]
fn take_film_takes_pro_rata() {
    let world = strip(&[1; 4]);
    // No grazing refuge here: this case is the pool rule, emptied to the last unit.
    let mut c = config();
    c.film_graze_refuge = 0.0;
    let mut flora = Flora::in_world(&world, c);
    let site = face(2);
    put_film(&mut flora, site, 0.01);
    let l0 = flora.view().ledger.clone();

    let a = flora.take_film(site, 0.004).expect("film to take");
    assert!((a.organic - 0.004).abs() <= 1e-18);
    assert!((a.mineral - 0.02 * 0.004).abs() <= 1e-15 * a.mineral);
    assert!((a.energy - 2.0 * 0.004).abs() <= 1e-15 * a.energy);
    let g = ground(&flora, site);
    assert!((g.film - 0.006).abs() <= 1e-15);
    assert!(
        (g.film_mineral / g.film - 0.02).abs() <= 1e-12,
        "the rest is the same stuff"
    );
    assert!((g.film_energy / g.film - 2.0).abs() <= 1e-12);

    let b = flora.take_film(site, 1.0).expect("the rest");
    let g = ground(&flora, site);
    assert_eq!(
        (g.film, g.film_mineral, g.film_energy),
        (0.0, 0.0, 0.0),
        "dust left"
    );
    assert!((a.organic + b.organic - 0.01).abs() <= 1e-15);
    assert!((a.mineral + b.mineral - 0.0002).abs() <= 1e-15);
    assert!((a.energy + b.energy - 0.02).abs() <= 1e-15);

    let l = flora.view().ledger.clone();
    assert_eq!(
        l.consumed_organic_out - l0.consumed_organic_out,
        a.organic + b.organic
    );
    assert_eq!(
        l.consumed_mineral_out - l0.consumed_mineral_out,
        a.mineral + b.mineral
    );
    assert_eq!(
        l.consumed_energy_out - l0.consumed_energy_out,
        a.energy + b.energy
    );
    assert_eq!(l.film_consumed - l0.film_consumed, a.organic + b.organic);

    assert_eq!(flora.take_film(site, 0.001), None, "an empty film");
    assert_eq!(flora.take_film(face(1), 0.001), None, "a site with no film");
    assert_eq!(
        flora.take_film(Site { x: 2, y: 3, z: 0 }, 0.001),
        None,
        "no ground"
    );
    put_film(&mut flora, site, 0.01);
    for want in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(flora.take_film(site, want), None, "want {want}");
    }
    assert_eq!(
        ground(&flora, site).film,
        0.01,
        "a refused take took something"
    );
    assert_closes(&flora, "grazed");
    assert_film_budget(&flora, "grazed");
}

// ------------------------------------------------------------------------ ledger

/// One short run with all of it — founding over mixed water, growth, spread, a graze, then
/// two faces drying — and the books close at every stage, the film's own terms included.
#[test]
fn the_ledger_closes_over_a_short_run_with_film() {
    // Eligible: 1, 2, 3 (0.5 m), 5, 7; column 4 is 0.75 m; 0 and 6 are dry.
    let mut world = strip(&[0, 1, 1, 2, 3, 1, 0, 1]);
    let mut flora = Flora::in_world(&world, config());
    assert_closes(&flora, "created");
    assert_eq!(flora.seed_film(&world), 5);
    let seed = flora.config().film_seed_per_m2 * AREA_M2;
    assert!((flora.view().ledger.film_seeded - 5.0 * seed).abs() <= 1e-15 * seed);
    assert_closes(&flora, "seeded");
    assert_film_budget(&flora, "seeded");

    run(&mut flora, &mut world, TICKS / 2);
    assert_closes(&flora, "grown");
    assert_film_budget(&flora, "grown");

    let half = 0.5 * ground(&flora, face(1)).film;
    let taken = flora.take_film(face(1), half).expect("film to graze");
    assert_eq!(taken.organic, half);
    assert_closes(&flora, "grazed");
    assert_film_budget(&flora, "grazed");

    // Columns 2 and 5 dry out.
    let mut drier = strip(&[0, 1, 0, 2, 3, 0, 0, 1]);
    run(&mut flora, &mut drier, TICKS / 2);
    let l = &flora.view().ledger;
    assert!(l.film_produced > 0.0, "nothing grew");
    assert!(l.film_to_litter > 0.0, "nothing died");
    assert!(l.film_consumed > 0.0, "nothing was eaten");
    assert_closes(&flora, "dried");
    assert_film_budget(&flora, "dried");
}

/// The pool is saved, under a new schema.
#[test]
fn the_film_survives_a_snapshot() {
    assert!(SCHEMA >= 11, "the ground record changed: schema {SCHEMA}");
    let world = strip(&[1; 4]);
    let mut flora = Flora::in_world(&world, config());
    put_film(&mut flora, face(2), 0.01);
    let back = decode(&encode(&flora)).expect("round trip");
    assert_eq!(
        back.view().ground_at(face(2)),
        flora.view().ground_at(face(2))
    );
    assert_eq!(back.view().ledger.film_seeded, 0.01);
}

// ------------------------------------------------------------ residues and drying out
//
// Wrysk, 2026-10-02: "it should definitely die when it dries out." A drained shower leaves
// a residue on every ledge (under an overhang nothing evaporates it), and the old rule
// read any water above 0 as wet; and a dry site's loss was exponential, so it kept a
// sliver forever. `film_wet_min_depth_m` makes a residue dry; `film_gone_fraction` ends a
// dry film at exactly nothing.

/// A one-deep strip like [`strip`], with `depths[x]` metres of water standing in the one
/// cell over column `x` (each under a cell).
fn strip_m(depths: &[f64]) -> World {
    let mut w = strip(&vec![0; depths.len()]);
    for (x, &d) in depths.iter().enumerate() {
        if d > 0.0 {
            assert!(d < VOXEL_M, "fixture: one cell at most");
            w.apply(WorldCommand::AddWater {
                x: x as i64,
                y: 1,
                z: 0,
                volume_m3: d * AREA_M2,
            });
        }
    }
    for (x, &d) in depths.iter().enumerate() {
        let got = w.view().standing_depth_m(x as i64, 0, 0);
        assert!(
            (got - d).abs() <= 1e-12 * d.max(1e-12),
            "fixture: column {x} stands {got} m, want {d}"
        );
    }
    w
}

/// The site capacity on the test strip.
fn capacity(c: &FloraConfig) -> f64 {
    c.film_capacity_per_m2 * AREA_M2
}

/// A site whose standing water is a residue below `film_wet_min_depth_m` is dry: not
/// eligible, no growth, dry death into its own litter, and it neither gives nor receives
/// spread. A site standing at the knob exactly keeps the old (wet) behaviour.
#[test]
fn a_residue_is_dry() {
    let c = config();
    let min = c.film_wet_min_depth_m;
    assert!(min > 0.0, "the knob is on by default");
    let residue = 0.5 * min;
    let wet = 2.0 * min;

    // Eligibility on its own: under the knob no, at and over it yes.
    let w = strip_m(&[residue, wet]);
    let v = w.view();
    assert!(!crate::film::eligible(&v, &c, 0, 0, 0), "a residue is eligible");
    assert!(crate::film::eligible(&v, &c, 1, 0, 0), "wet water is not eligible");
    let at = FloraConfig {
        film_wet_min_depth_m: v.standing_depth_m(1, 0, 0),
        ..c.clone()
    };
    assert!(crate::film::eligible(&v, &at, 1, 0, 0), "at the knob is not eligible");

    // Growth and death: a seeded film over a residue declines into its litter, grows
    // nothing.
    let mut flora = Flora::in_world(&w, FloraConfig { decomposition: 0.0, ..config() });
    let f0 = 0.5 * capacity(flora.config());
    put_film(&mut flora, face(0), f0);
    let litter0 = ground(&flora, face(0)).litter;
    let mut world = strip_m(&[residue, 0.0, 0.0, 0.0]);
    run(&mut flora, &mut world, TICKS);
    let g = ground(&flora, face(0));
    assert!(g.film < f0, "the residue's film did not decline");
    assert!(g.litter > litter0, "the residue's film died to no litter");
    let l = &flora.view().ledger;
    assert_eq!(l.film_produced, 0.0, "film grew on a residue");
    assert!(l.film_to_litter > 0.0, "no dry death on a residue");
    assert_closes(&flora, "residue");
    assert_film_budget(&flora, "residue");

    // Spread, no growth or upkeep: column 0 is wet and filmed, 1 a bare residue beside
    // it; 2 a filmed residue, 3 bare wet water beside it; 4 and 5 dry (the ring wraps).
    // Neither residue gives or receives.
    let mut world = strip_m(&[wet, residue, residue, wet, 0.0, 0.0]);
    let mut flora = Flora::in_world(
        &world,
        FloraConfig {
            film_growth_per_s: 0.0,
            film_maintenance_per_s: 0.0,
            film_dry_death_per_s: 0.0,
            film_gone_fraction: 0.0,
            ..config()
        },
    );
    put_film(&mut flora, face(0), 0.01);
    put_film(&mut flora, face(2), 0.01);
    run(&mut flora, &mut world, TICKS);
    assert_eq!(ground(&flora, face(1)).film, 0.0, "a residue received spread");
    assert_eq!(ground(&flora, face(3)).film, 0.0, "a residue gave spread");
    assert_eq!(ground(&flora, face(0)).film, 0.01, "the wet source spread onto a residue");
    assert_eq!(ground(&flora, face(2)).film, 0.01, "the residue lost film to spread");
    assert_closes(&flora, "residue spread");
    assert_film_budget(&flora, "residue spread");

    // At the knob exactly: the old behaviour, growth and spread both.
    let mut world = strip_m(&[wet, wet, 0.0, 0.0]);
    let d = world.view().standing_depth_m(0, 0, 0);
    let mut flora = Flora::in_world(
        &world,
        FloraConfig {
            film_wet_min_depth_m: d,
            ..config()
        },
    );
    let f0 = 0.1 * capacity(flora.config());
    put_film(&mut flora, face(0), f0);
    run(&mut flora, &mut world, TICKS);
    assert!(flora.view().ledger.film_produced > 0.0, "at the knob: no growth");
    assert!(ground(&flora, face(1)).film > 0.0, "at the knob: no spread");
    assert_eq!(flora.view().ledger.film_to_litter, 0.0, "at the knob: dry death");
    assert_closes(&flora, "at the knob");
    assert_film_budget(&flora, "at the knob");
}

/// On a dry site the film falls until the step it would fall under `film_gone_fraction`
/// of the site's capacity, and in that step all of it dies into the site's litter: film,
/// its mineral and its energy exactly 0, the site gone from `film_sites`, the books shut.
/// (A faster death rate than the default so the test is short; the rule is the same.)
#[test]
fn dry_film_dies_out_completely() {
    let wet = strip(&[1; 3]);
    let mut flora = Flora::in_world(
        &wet,
        FloraConfig {
            decomposition: 0.0,
            film_dry_death_per_s: 0.05,
            ..config()
        },
    );
    let c = flora.config().clone();
    let floor = c.film_gone_fraction * capacity(&c);
    assert!(floor > 0.0, "the floor is on by default");
    let f0 = 0.5 * capacity(&c);
    put_film(&mut flora, face(1), f0);
    assert_closes(&flora, "placed");
    let mut dry = strip(&[0; 3]);
    // ln(f0 / floor) / rate, and a margin.
    let need_s = (f0 / floor).ln() / (c.film_dry_death_per_s + c.film_maintenance_per_s);
    let ticks = ((need_s * 1.5) / DT).ceil() as u32 + 2 * TICK_HZ;
    let mut gone_at = None;
    let mut last = f0;
    for t in 0..ticks {
        flora.step(&mut dry);
        let g = ground(&flora, face(1));
        if g.film > 0.0 {
            assert!(
                g.film >= floor,
                "tick {t}: a dry film stands at {} under the floor {floor}",
                g.film
            );
            assert!(g.film <= last, "tick {t}: a dry film grew");
            last = g.film;
        } else if gone_at.is_none() {
            gone_at = Some(t);
            let litter_film = flora.view().ledger.film_to_litter;
            assert!(litter_film > 0.0);
        }
    }
    let at = gone_at.unwrap_or_else(|| panic!("a dry film never died out ({last} left)"));
    assert!(
        f64::from(at) * DT <= need_s * 1.5,
        "died out at {} s, the law gives about {need_s:.0} s",
        f64::from(at) * DT
    );
    let g = ground(&flora, face(1));
    assert_eq!((g.film, g.film_mineral, g.film_energy), (0.0, 0.0, 0.0), "dust left");
    assert!(g.litter > 0.0, "the last of it went nowhere");
    assert_eq!(flora.view().film_sites().count(), 0, "a dried site is still a film site");
    let l = &flora.view().ledger;
    assert!(
        (l.film_to_litter + l.film_respired - f0).abs() <= 1e-12 * f0,
        "film {f0} went {} to litter and {} to respiration",
        l.film_to_litter,
        l.film_respired
    );
    assert_closes(&flora, "died out");
    assert_film_budget(&flora, "died out");
}

/// The floor is for dry sites only: a wet, eligible, lit site holding less film than the
/// floor keeps it and grows.
#[test]
fn the_floor_never_touches_wet_film() {
    let mut world = strip(&[0, 1, 0]);
    let mut flora = Flora::in_world(&world, config());
    let c = flora.config().clone();
    let f0 = 0.25 * c.film_gone_fraction * capacity(&c);
    assert!(f0 > 0.0);
    put_film(&mut flora, face(1), f0);
    run(&mut flora, &mut world, TICKS);
    let f = ground(&flora, face(1)).film;
    assert!(f > f0, "a wet film under the floor holds {f}, placed {f0}");
    assert_eq!(flora.view().ledger.film_to_litter, 0.0, "wet film died");
    assert_closes(&flora, "wet under the floor");
    assert_film_budget(&flora, "wet under the floor");
}

/// The knobs: serde defaults (a config that never names them gets them) and registry rows.
#[test]
fn the_drying_knobs_are_plumbed() {
    let d = FloraConfig::default();
    let from: FloraConfig = toml::from_str("").expect("an empty flora table");
    assert_eq!(from.film_wet_min_depth_m, d.film_wet_min_depth_m);
    assert_eq!(from.film_gone_fraction, d.film_gone_fraction);
    let set: FloraConfig =
        toml::from_str("film_wet_min_depth_m = 0.003\nfilm_gone_fraction = 0.01\n").expect("set");
    assert_eq!((set.film_wet_min_depth_m, set.film_gone_fraction), (0.003, 0.01));
    for path in ["flora.film_wet_min_depth_m", "flora.film_gone_fraction"] {
        assert!(
            crate::registry().rows().iter().any(|r| r.path == path),
            "no registry row for {path}"
        );
    }
    // The floor is under the render's half level (marks are drawn from 1/16 of capacity),
    // and the residue threshold well under a one-cell pond margin.
    assert!(d.film_gone_fraction > 0.0 && d.film_gone_fraction < 1.0 / 16.0);
    assert!(d.film_wet_min_depth_m > 0.0 && d.film_wet_min_depth_m < 0.25 * 0.125);
    d.validate().expect("the defaults validate");
}

/// The real water step: a pond beside a stepped slope rising to a plateau whose middle is
/// roofed, the slope stepping back down to the pond (the strip is a ring). Film on every
/// face, a shower, the rain stops, the water drains. Within a bound set by the knobs every
/// slope, plateau, covered-ledge and roof face holds no film and is out of `film_sites`;
/// the pond's faces keep theirs.
#[test]
fn rain_then_drain_leaves_film_only_in_the_pond() {
    const V: f64 = 0.125;
    // Column tops (the support face's y); 0 is the pond floor.
    let tops: [u32; 16] = [0, 0, 0, 0, 2, 3, 4, 5, 5, 5, 5, 4, 3, 2, 2, 2];
    let roof = (8u32, 8..=9i64);
    let mut world = World::empty(VoxelConfig {
        width: 16,
        height: 12,
        depth: 1,
        voxel_m: V,
        seed: 23,
        evaporation_m_per_s: 1e-5,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        ..VoxelConfig::default()
    });
    for (x, &top) in tops.iter().enumerate() {
        for y in 1..=top {
            world.apply(WorldCommand::SetMaterial {
                x: x as i64,
                y,
                z: 0,
                material: Material::Bedrock,
            });
        }
    }
    for x in roof.1.clone() {
        world.apply(WorldCommand::SetMaterial {
            x,
            y: roof.0,
            z: 0,
            material: Material::Bedrock,
        });
    }
    let cell = world.config().voxel_volume();
    for x in 0..4 {
        world.apply(WorldCommand::AddWater {
            x,
            y: 1,
            z: 0,
            volume_m3: 0.95 * cell,
        });
    }
    let mut flora = Flora::in_world(&world, config());
    let c = flora.config().clone();
    let cap = c.film_capacity_per_m2 * V * V;
    let mut faces = Vec::new();
    for x in 0..16i64 {
        for y in 0..12u32 {
            if world.view().is_support(x, y, 0) {
                faces.push(Site { x: x as u32, y, z: 0 });
            }
        }
    }
    let pond: Vec<Site> = (0..4).map(|x| Site { x, y: 0, z: 0 }).collect();
    for &s in &faces {
        put_film(&mut flora, s, 0.5 * cap);
    }
    assert!(faces.len() > pond.len() + 12, "fixture: {} faces", faces.len());

    // A five-minute shower, about 60 mm/h, through the real rain distribution.
    let hz = u64::from(TICK_HZ);
    let rate = 1.7e-5;
    for _ in 0..5 * 60 * hz {
        world.apply(WorldCommand::RainPulse {
            volume_m3: rate * DT * 16.0 * V * V,
        });
        world.step();
        flora.step(&mut world);
    }
    let wet_ledges = faces
        .iter()
        .filter(|s| !pond.contains(s) && world.view().standing_depth_m(i64::from(s.x), s.y, 0) > 0.0)
        .count();
    assert!(wet_ledges > 0, "fixture: the shower wetted no ledge");

    // The bound: five minutes to drain, then a full film's dry death to the floor.
    let bound_s = 300.0 + (1.0 / c.film_gone_fraction).ln() / c.film_dry_death_per_s;
    assert!(bound_s <= 20.0 * 60.0, "the knobs give {bound_s:.0} s");
    for _ in 0..(bound_s * f64::from(TICK_HZ)).ceil() as u64 {
        world.step();
        flora.step(&mut world);
    }
    let v = flora.view();
    let left: Vec<(Site, f64, f64)> = v
        .film_sites()
        .filter(|(s, _)| !pond.contains(s))
        .map(|(s, f)| (s, f / cap, world.view().standing_depth_m(i64::from(s.x), s.y, 0)))
        .collect();
    assert!(
        left.is_empty(),
        "{bound_s:.0} s after the rain, film (site, share of capacity, depth m) on {left:?}"
    );
    for &s in &pond {
        let f = v.film_at(s);
        assert!(f > 0.25 * cap, "pond face {s:?} kept {f} of {cap}");
    }
    assert_closes(&flora, "rain and drain");
    assert_film_budget(&flora, "rain and drain");
}

// ---------------------------------------------------------------- land-wet ground

/// **Glassfilm on wet ground** (SNAIL-FIX, [`FloraConfig::film_land_moisture_min`]): film
/// grows on a face with no water over it only while the face's pore fill is at or above
/// the threshold, dies below it, and stands as a ground pool beside the face's plant, not
/// in its place. A four-column soil strip (soil `y = 1..=2`, face `y = 2`), no water: pore
/// fill above the threshold at columns 0 and 3, below it at 1, exactly at it at 2 (read by
/// the classifier only);
/// springturf on column 3; a quarter of the land capacity of film on every face.
#[test]
fn land_film_grows_only_at_or_above_its_wetness_threshold_and_takes_no_cover_slot() {
    let c = config();
    let thr = c.film_land_moisture_min;
    assert!(thr > 0.0 && thr < 1.0, "the shipped threshold is a pore fill inside 0..1");
    let pores = [0.5 * (thr + 1.0), 0.5 * thr, thr, 0.5 * (thr + 1.0)];
    let mut world = World::empty(VoxelConfig {
        width: pores.len() as u32,
        height: 6,
        depth: 1,
        voxel_m: VOXEL_M,
        seed: 23,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        ..VoxelConfig::default()
    });
    let cap = world.config().materials.pore_capacity(Material::Soil) * world.config().voxel_volume();
    for (x, &p) in pores.iter().enumerate() {
        let x = x as i64;
        for y in 1..=2u32 {
            world.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Air });
            world.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: p * cap });
            world.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
    let at = |x: u32| Site { x, y: 2, z: 0 };
    let v = world.view();
    for (x, &p) in pores.iter().enumerate() {
        let got = v.pore_at(x as i64, 2, 0);
        assert!((got - p).abs() < 1e-9, "fixture: column {x} pore fill {got}, want {p}");
        assert_eq!(v.standing_depth_m(x as i64, 2, 0), 0.0, "fixture: no water on column {x}");
    }
    assert_eq!(crate::film_wet(&v, &c, 0, 2, 0), crate::FilmWet::Land);
    assert_eq!(crate::film_wet(&v, &c, 1, 2, 0), crate::FilmWet::Dry);
    assert_eq!(crate::film_wet(&v, &c, 2, 2, 0), crate::FilmWet::Land, "at the threshold is wet");

    let mut flora = Flora::in_world(&world, c.clone());
    let wood = flora.config.species(crate::Species::Springturf).wood_max;
    assert!(flora.apply(
        &world,
        crate::Command::SeedOnFace { site: at(3), species: crate::Species::Springturf, wood }
    ));
    let start = 0.25 * c.film_land_capacity_per_m2 * AREA_M2;
    for x in 0..pores.len() as u32 {
        put_film(&mut flora, at(x), start);
    }
    let stands_before: Vec<_> = flora.view().stands.iter().map(|s| (s.site, s.species)).collect();
    run(&mut flora, &mut world, TICKS);

    let film = |x: u32| flora.view().film_at(at(x));
    assert!(film(0) > start, "above the threshold it grows: {} from {start}", film(0));
    // Column 2 sits exactly at the threshold: the classifier above says it is wet; the
    // plant layer's own water draw may take it under in the run, so its film is not read.
    assert!(film(1) < start, "below the threshold it dies: {} from {start}", film(1));
    assert!(film(3) > 0.0, "it stands on the springturf's face too");
    let stands_after: Vec<_> = flora.view().stands.iter().map(|s| (s.site, s.species)).collect();
    assert_eq!(stands_after, stands_before, "the film takes no plant's place and adds none");
    assert_eq!(flora.view().stands_on(at(3)).len(), 1, "the springturf keeps its face");
    assert_film_budget(&flora, "land film");
    assert_closes(&flora, "land film");
}

// ---------------------------------------------------------------- the shore (SNAIL-LAND)

/// A one-deep strip for the shore: column 0 a pond (bedrock face at `y = 0`, two cells of
/// water over it), every other column soil at `y = 1..=2` (face `y = 2`) holding the given
/// pore fill and no water over it.
fn shore_strip(pores: &[f64]) -> World {
    let mut world = World::empty(VoxelConfig {
        width: pores.len() as u32 + 1,
        height: 6,
        depth: 1,
        voxel_m: VOXEL_M,
        seed: 29,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        ..VoxelConfig::default()
    });
    let cap = world.config().materials.pore_capacity(Material::Soil) * world.config().voxel_volume();
    for (i, &p) in pores.iter().enumerate() {
        let x = i as i64 + 1;
        for y in 1..=2u32 {
            world.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Air });
            world.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: p * cap });
            world.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
    let cell = world.config().voxel_volume();
    for y in 1..=2u32 {
        world.apply(WorldCommand::AddWater { x: 0, y, z: 0, volume_m3: cell });
    }
    let v = world.view();
    assert!(v.standing_depth_m(0, 0, 0) > 0.0, "fixture: the pond stands");
    for (i, &p) in pores.iter().enumerate() {
        let x = i as i64 + 1;
        assert!((v.pore_at(x, 2, 0) - p).abs() < 1e-9, "fixture: column {x} pore fill");
        assert_eq!(v.standing_depth_m(x, 2, 0), 0.0, "fixture: no water on column {x}");
    }
    world
}

/// **Shore film** (SNAIL-LAND, [`FloraConfig::film_shore_band_m`],
/// [`FloraConfig::film_shore_moisture_min`], [`FloraConfig::film_land_capacity_wet_per_m2`]):
/// a dry-surfaced face in the shore band is land-wet from the shore's floor, the same pore
/// fill out of the band is dry, and a land-wet face's capacity runs linearly with pore fill
/// from the land capacity at its floor to the wet capacity at saturation. Every number is
/// set here; the run checks that film grows in the band and dies out of it, and stops at
/// the face's own capacity.
#[test]
fn land_film_capacity_rises_with_moisture_and_the_shore_band_admits_damp_bank() {
    let mut c = config();
    c.film_land_moisture_min = 0.9;
    c.film_shore_moisture_min = 0.5;
    c.film_shore_band_m = 2.2 * VOXEL_M; // columns 1 and 2 are in it, 3 on are not
    c.film_land_capacity_per_m2 = 0.2;
    c.film_land_capacity_wet_per_m2 = 0.4;
    // Columns 1..=6: in-band damp, in-band too dry, out-of-band damp, saturated,
    // between the land floor and saturation, out-of-band too dry.
    let pores = [0.6, 0.4, 0.6, 1.0, 0.95, 0.4];
    let mut world = shore_strip(&pores);
    let at = |x: u32| Site { x, y: 2, z: 0 };
    {
        let v = world.view();
        assert!(crate::shore_band(&v, c.film_shore_band_m, 1, 2, 0));
        assert!(crate::shore_band(&v, c.film_shore_band_m, 2, 2, 0));
        assert!(!crate::shore_band(&v, c.film_shore_band_m, 3, 2, 0));
        let wet = |x: i64| crate::film_wet(&v, &c, x, 2, 0);
        assert_eq!(wet(1), crate::FilmWet::Land, "damp bank in the band");
        assert_eq!(wet(2), crate::FilmWet::Dry, "under the shore floor");
        assert_eq!(wet(3), crate::FilmWet::Dry, "damp, out of the band");
        assert_eq!(wet(4), crate::FilmWet::Land);
        assert_eq!(wet(5), crate::FilmWet::Land);
        assert_eq!(wet(6), crate::FilmWet::Dry);
        let mut no_band = c.clone();
        no_band.film_shore_band_m = 0.0;
        assert_eq!(crate::film_wet(&v, &no_band, 1, 2, 0), crate::FilmWet::Dry, "band 0: no shore");
    }
    // The capacity: linear in pore fill from each face's floor.
    let k = |m: f64, floor: f64| crate::land_capacity_per_m2(&c, m, floor);
    let lerp = |m: f64, floor: f64| {
        c.film_land_capacity_per_m2
            + (c.film_land_capacity_wet_per_m2 - c.film_land_capacity_per_m2) * (m - floor) / (1.0 - floor)
    };
    assert!((k(0.6, 0.5) - lerp(0.6, 0.5)).abs() < 1e-12);
    assert!((k(0.95, 0.9) - lerp(0.95, 0.9)).abs() < 1e-12);
    assert!((k(1.0, 0.9) - c.film_land_capacity_wet_per_m2).abs() < 1e-12);
    assert!((k(0.9, 0.9) - c.film_land_capacity_per_m2).abs() < 1e-12);
    assert!(k(0.95, 0.9) > k(0.9, 0.9) && k(1.0, 0.9) > k(0.95, 0.9), "rises with moisture");
    let mut flat = c.clone();
    flat.film_land_capacity_wet_per_m2 = flat.film_land_capacity_per_m2;
    assert!((crate::land_capacity_per_m2(&flat, 1.0, 0.9) - flat.film_land_capacity_per_m2).abs() < 1e-12, "equal knobs: flat");

    // A short run: film grows on the damp bank in the band and dies out of it; on the
    // saturated face film over its (wet) capacity is held to it, not the land floor's.
    c.film_spread_per_s = 0.0;
    let mut flora = Flora::in_world(&world, c.clone());
    let small = 0.1 * c.film_land_capacity_per_m2 * AREA_M2;
    for x in [1, 3] {
        put_film(&mut flora, at(x), small);
    }
    let wet_cap = c.film_land_capacity_wet_per_m2 * AREA_M2;
    put_film(&mut flora, at(4), 0.9 * wet_cap);
    run(&mut flora, &mut world, TICKS);
    let film = |x: u32| flora.view().film_at(at(x));
    assert!(film(1) > small, "in the band it grows: {} from {small}", film(1));
    assert!(film(3) < small, "out of the band it dies: {} from {small}", film(3));
    assert!(
        film(4) > c.film_land_capacity_per_m2 * AREA_M2,
        "saturated ground holds film above the floor's capacity: {}",
        film(4)
    );
    assert_film_budget(&flora, "shore film");
    assert_closes(&flora, "shore film");
}

/// **The shore knobs round-trip** (SNAIL-LAND): the config is in the snapshot.
#[test]
fn the_shore_knobs_survive_a_snapshot() {
    let world = strip(&[1; 4]);
    let mut c = config();
    c.film_shore_band_m = 0.75;
    c.film_shore_moisture_min = 0.55;
    c.film_land_capacity_wet_per_m2 = 0.33;
    let flora = Flora::in_world(&world, c.clone());
    let back = decode(&encode(&flora)).expect("round trip");
    assert_eq!(back.config.film_shore_band_m, c.film_shore_band_m);
    assert_eq!(back.config.film_shore_moisture_min, c.film_shore_moisture_min);
    assert_eq!(back.config.film_land_capacity_wet_per_m2, c.film_land_capacity_wet_per_m2);
}

/// **TASTE** (Wrysk 2026-10-06: "why does empty face still taste like algae?"): what a
/// grazer perceives of a face's film is what [`Flora::take_film`] would give it, the film
/// above the grazing refuge (`film_graze_refuge` of a site's capacity), read through the one
/// accessor ([`crate::FloraView::edible_film_at`], [`crate::FloraView::edible_film`]). On
/// either side of the refuge, at it, and with no film, the reading equals the whole take,
/// and a face at or under its refuge reads 0.
#[test]
fn the_edible_film_a_view_reads_is_what_take_film_gives() {
    let world = strip(&[1; 4]);
    let c = config();
    let refuge = c.film_graze_refuge * c.film_capacity_per_m2 * c.voxel_m * c.voxel_m;
    assert!(refuge > 0.0, "fixture: a refuge");
    for stock in [0.0, 0.5 * refuge, refuge, 1.5 * refuge, 10.0 * refuge] {
        let mut flora = Flora::in_world(&world, c.clone());
        let site = face(1);
        if stock > 0.0 {
            put_film(&mut flora, site, stock);
        }
        let at = flora.view().edible_film_at(site);
        let of_ground = flora.view().edible_film(&ground(&flora, site));
        let took = flora.take_film(site, 1.0e9).map_or(0.0, |t| t.organic);
        assert_eq!(at, took, "film {stock}: the view reads {at}, a mouth takes {took}");
        assert_eq!(of_ground, took, "film {stock}: per ground entry {of_ground}, a mouth takes {took}");
        if stock <= refuge {
            assert_eq!(at, 0.0, "film {stock} at or under the refuge {refuge} is not food");
        } else {
            assert!(at > 0.0, "film {stock} above the refuge {refuge} is food");
        }
    }
}
