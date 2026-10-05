//! A seedling's life must not depend on its species' adult foliage capacity `alpha`
//! (`turf_trace`, 2026-10-04: at springturf preset alpha 6.3 the day reflush spent every
//! newborn's seed reserve on leaves, and a newborn at `alive_min` with no reserve dies on
//! its first unpaid upkeep). A child of `step`, so a case grows one hand-built stand
//! under a climate it sets. No tuned number is pinned: every expectation is read from the
//! config.

use super::*;
use crate::seeds::{newborn_stocks, package_of};
use crate::{Command, Flora, FloraConfig, Ground, Species, Stand};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

const TURF: Species = Species::Springturf;

/// The default table with the temperature factor off, curing off and nothing reproducing,
/// so `grow_stand` is the only thing moving the stand.
fn config(alpha: f64) -> FloraConfig {
    let mut c = FloraConfig::default();
    c.upkeep_q10 = 1.0;
    let sc = c.species_mut(TURF);
    sc.alpha = alpha;
    sc.cure_after_s = 0.0;
    sc.propagule_rate = 0.0;
    c
}

/// A soil slab under open sky, wet.
fn slab() -> World {
    let mut w = World::empty(VoxelConfig {
        width: 3,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 11,
        ..VoxelConfig::default()
    });
    for x in 0..3i64 {
        for y in 1..=2u32 {
            let want = 0.6 * w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
            w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: want });
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
    w
}

/// A stand of `wood` seeded on the slab, cloned out with a ground slot rich in mineral
/// (so the Monod factor is close to one and the draw caps never bind).
fn seeded(config: &FloraConfig, wood: f64) -> (Stand, Ground) {
    let world = slab();
    let mut flora = Flora::new(config.clone());
    assert!(flora.apply(&world, Command::Seed { x: 1, z: 0, species: TURF, wood }));
    (flora.stands[0], Ground::new(flora.stands[0].site, 1.0e3))
}

/// A newborn exactly as germination builds one out of one minimum package.
fn newborn(config: &FloraConfig) -> (Stand, Ground) {
    let sc = config.species(TURF);
    let (wood, foliage, reserve) = newborn_stocks(sc, package_of(sc));
    let (mut s, g) = seeded(config, wood);
    s.wood = wood;
    s.foliage = foliage;
    s.reserve = reserve;
    s.bin_layers(config);
    (s, g)
}

/// Full day, a moist root, no stress.
fn day() -> Climate {
    Climate {
        daylight: 1.0,
        upkeep: 1.0,
        night: 1.0,
        to_dawn_s: None,
        draw_key: 0,
        dt: crate::DT,
    }
}

fn drink() -> Drink {
    Drink {
        moisture: 1.0,
        taken_m3: 0.0,
        saturated: 0.0,
        darkness: 0.0,
        available: 1.0,
    }
}

/// The organic matter `s` and its site hold: stand, litter, dead wood.
fn held(s: &Stand, g: &Ground) -> f64 {
    s.material() + g.litter + g.dead_wood
}

/// One tick, checking organic conservation against the tick's own books: what the stand
/// and its site hold changes by what it fixed less what it respired.
fn tick(config: &FloraConfig, s: &mut Stand, g: &mut Ground, light: f64) -> Option<DeathCause> {
    let before = held(s, g);
    let mut book = Booked::default();
    let cause = grow_stand_cause(config, s, g, light, day(), drink(), Substrate::default(), &mut book);
    let respired: f64 = book.respired[..book.respired_n as usize].iter().sum();
    let resid = held(s, g) - (before + book.fixed + book.seep[0] - respired);
    assert!(resid.abs() <= 1e-12 * before.max(1.0), "organic residual {resid}");
    cause
}

/// The light at which a newborn's income is `share` of its maintenance (Monod at the
/// fixture's mineral, moisture one), read from the config.
fn light_at(config: &FloraConfig, s: &Stand, g: &Ground, share: f64) -> f64 {
    let sc = config.species(TURF);
    let monod = g.mineral / (g.mineral + sc.nutrient_half);
    share * sc.maintenance * s.wood / (sc.assimilation * s.foliage * monod)
}

/// Two alphas around the newborn: both carry its foliage (`alpha · W > P`), and
/// `reflush_below · alpha · W` is below its foliage at the first and above it at the second.
fn alphas() -> (f64, f64) {
    let c = FloraConfig::default();
    let sc = c.species(TURF);
    let (w, p, _) = newborn_stocks(sc, package_of(sc));
    let ratio = p / w;
    let above = ratio / sc.reflush_below; // the alpha at which the threshold meets P
    let lo = (ratio * above).sqrt().max(ratio * 1.01);
    let hi = 2.0 * above;
    assert!(lo * w > p && sc.reflush_below * lo * w < p, "fixture: threshold below the newborn");
    assert!(sc.reflush_below * hi * w > p, "fixture: threshold above the newborn");
    (lo, hi)
}

/// Run a newborn at `light` until it dies or `max` ticks pass: (ticks lived, reserve after
/// `probe` ticks).
fn life(alpha: f64, share: f64, probe: u64, max: u64) -> (u64, f64) {
    let c = config(alpha);
    let (mut s, mut g) = newborn(&c);
    let light = light_at(&c, &s, &g, share);
    let mut at_probe = f64::NAN;
    for t in 1..=max {
        let died = tick(&c, &mut s, &mut g, light);
        if t == probe {
            at_probe = s.reserve;
        }
        if died.is_some() {
            return (t, at_probe);
        }
    }
    (max, at_probe)
}

/// The defect itself: below the newborn's break-even light, how long it lives on its seed
/// reserve, and what is left of the reserve partway, do not depend on whether `alpha` puts
/// the reflush threshold above or below its foliage.
#[test]
fn a_newborns_survival_and_reserve_do_not_depend_on_alpha() {
    let (lo, hi) = alphas();
    let c = config(lo);
    let sc = c.species(TURF);
    let (s, _) = newborn(&c);
    // Upper bound on its life at no income at all: the reserve over the upkeep, with room.
    let max = (4.0 * s.reserve / (sc.maintenance * s.wood * crate::DT)).ceil() as u64;
    let (life_lo, _) = life(lo, 0.5, 1, max);
    assert!(life_lo < max, "fixture: below break-even it starves");
    let probe = life_lo / 2;
    let (life_lo, q_lo) = life(lo, 0.5, probe, max);
    let (life_hi, q_hi) = life(hi, 0.5, probe, max);
    assert!(
        life_lo.abs_diff(life_hi) <= 1,
        "lived {life_lo} ticks at alpha {lo:.3}, {life_hi} at {hi:.3}"
    );
    // One tick's upkeep at the newborn's wood is the step's resolution on the reserve.
    let tol = sc.maintenance * s.wood * crate::DT;
    assert!(
        (q_lo - q_hi).abs() <= tol,
        "reserve after {probe} ticks: {q_lo:.3e} at alpha {lo:.3}, {q_hi:.3e} at {hi:.3}"
    );
}

/// A newborn grazed below `reflush_below` of the leaf it was born with regrows toward that
/// leaf from its reserve, the same at both alphas, and no further.
#[test]
fn a_grazed_seedling_regrows_from_its_reserve_toward_its_birth_leaf() {
    let (lo, hi) = alphas();
    let mut out = Vec::new();
    for alpha in [lo, hi] {
        let c = config(alpha);
        let sc = c.species(TURF);
        let (mut s, mut g) = newborn(&c);
        let (w0, p0, q0) = (s.wood, s.foliage, s.reserve);
        let target = sc.reflush_below * p0;
        s.foliage = 0.2 * target;
        s.bin_layers(&c);
        let start = s.foliage;
        // No light: every leaf it builds comes from the reserve.
        for _ in 0..200 {
            assert!(tick(&c, &mut s, &mut g, 0.0).is_none(), "fixture: it lives");
        }
        assert!(s.foliage > start, "alpha {alpha}: the seedling reflushed");
        assert!(s.reserve < q0, "alpha {alpha}: out of its reserve");
        let slack = sc.foliage_rate * w0 * crate::DT;
        assert!(
            s.foliage <= target + slack,
            "alpha {alpha}: regrew to {:.3e}, past reflush_below of its birth leaf {target:.3e}",
            s.foliage
        );
        out.push((s.foliage, s.reserve));
    }
    // Floating-point resolution only: nothing in the seedling's tick reads alpha.
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * a.abs().max(b.abs());
    assert!(close(out[0].0, out[1].0), "foliage {out:?}");
    assert!(close(out[0].1, out[1].1), "reserve {out:?}");
}

/// A mature stand's reflush is unchanged: grazed below `reflush_below · alpha · W` in the
/// day with no light to earn by, it rebuilds foliage from its reserve, tick by tick at
/// `min(foliage_rate · W · dt, reflush_below · alpha · W − P)`, paying construction.
#[test]
fn a_mature_stand_reflushes_from_its_reserve_as_before() {
    let (_, hi) = alphas();
    let c = config(hi);
    let sc = c.species(TURF);
    let (mut s, mut g) = seeded(&c, sc.wood_max);
    assert!(s.wood >= sc.donor_min, "fixture: mature");
    s.reserve = sc.reserve_cap * s.wood;
    s.foliage = 0.1 * sc.reflush_below * sc.alpha * s.wood;
    s.bin_layers(&c);
    let start = s.foliage;
    for _ in 0..50 {
        let (w, p, q) = (s.wood, s.foliage, s.reserve);
        let m = sc.maintenance * w * crate::DT;
        let want = (sc.foliage_rate * w * crate::DT).min((sc.reflush_below * sc.alpha * w - p).max(0.0));
        let shed = sc.senescence * (p + want) * crate::DT;
        assert!(tick(&c, &mut s, &mut g, 0.0).is_none());
        assert!((s.foliage - (p + want - shed)).abs() <= 1e-12, "foliage {} vs {}", s.foliage, p + want - shed);
        assert!((q - s.reserve - (m + (1.0 + sc.build) * want)).abs() <= 1e-12, "reserve paid");
    }
    assert!(s.foliage > start, "it regrew");
}
