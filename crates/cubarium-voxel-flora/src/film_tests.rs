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
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};

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

// ------------------------------------------------------------------------- the knobs

/// The brief's defaults: 0.6 m and 0.8 /m as given; a seeded film fills its capacity in
/// about an hour of good light and dies within about ten minutes of drying; it seeds below
/// capacity and spreads.
#[test]
fn film_defaults_meet_the_brief() {
    for c in [FloraConfig::default(), FloraConfig::for_voxel_size(0.125)] {
        assert_eq!(c.film_max_depth_m, 0.6);
        assert_eq!(c.film_extinction_per_m, 0.8);
        let k = c.film_capacity_per_m2;
        let s = c.film_seed_per_m2;
        assert!(
            k > 0.0 && s > 0.0 && s < 0.1 * k,
            "seed {s} against capacity {k}"
        );
        assert!(c.film_spread_per_s > 0.0);
        // Logistic with upkeep: net rate r − m, capacity K(1 − m/r); time from the seed
        // to 90 % of that capacity at full light.
        let r = c.film_growth_per_s - c.film_maintenance_per_s;
        assert!(r > 0.0, "a lit film must outgrow its upkeep");
        let cap = k * (1.0 - c.film_maintenance_per_s / c.film_growth_per_s);
        let fill_s = (9.0 * (cap - s) / s).ln() / r;
        assert!(
            (1800.0..=7200.0).contains(&fill_s),
            "a film fills in {:.0} min, want about an hour",
            fill_s / 60.0
        );
        // 95 % gone after drying.
        let die_s = 20f64.ln() / c.film_dry_death_per_s;
        assert!(
            (300.0..=1200.0).contains(&die_s),
            "a dried film is 95 % gone in {:.1} min, want about ten",
            die_s / 60.0
        );
    }
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
    let mut flora = Flora::in_world(&world, config());
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
