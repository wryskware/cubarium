//! **Drought dormancy on the budget** (package DORMANCY, 2026-10-05; Wrysk: "grass should go
//! dormant in drought and not starve"): a green stand whose water can no longer pay its
//! upkeep and leaf turnover cures instead of starving, a cured stand wakes at
//! `cure_resume_water` and regrows from its reserve, a stand whose water pays never cures,
//! and the ledgers close through a cure and a wake.
//!
//! Written before the rule. Every expected value is read from the config or built by hand
//! from the stand law's own break-even ([`crate::break_even_cover`]): no tuned number is
//! pinned. The single-stand cases run the sim's own growth step through
//! [`crate::study::probe_traced`] under a constant full sun; the ledger case runs the world
//! step on a three-column slab.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, Materials, World};

use crate::study::{ClimateSample, Held, Start, probe_traced, start_stand};
use crate::{
    Command, CureRead, DeathCause, Flora, FloraConfig, Site, Species, SpeciesConfig, Stand, break_even_cover, water_short,
};

const TURF: Species = Species::Springturf;

/// A site mineral high enough that the Monod factor is near one: water and light decide.
const RICH: f64 = 50.0;

/// The springturf of the code defaults, its curing band made explicit: the old absolute
/// clock (`cure_water`) at the wilting point, so it never runs inside the band; waking at
/// field capacity of the ramp (`full_water`), so the whole ramp below it is the dry band;
/// no dormancy cap, so only the reserve and the crown limit a dormant stand.
fn band_config() -> FloraConfig {
    let mut c = FloraConfig::default();
    let t = &mut c.springturf;
    t.cure_water = t.wilt_water;
    t.cure_resume_water = t.full_water;
    t.cure_max_s = 0.0;
    t.propagule_rate = 0.0;
    assert!(t.cure_after_s > 0.0, "springturf cures");
    assert!(t.cure_cover > 0.0, "the budget clock is on in the defaults");
    c
}

fn monod(sc: &SpeciesConfig, n: f64) -> f64 {
    n / (n + sc.nutrient_half)
}

/// The light response at raw light 1 (open sky, no canopy).
fn l_open(sc: &SpeciesConfig) -> f64 {
    crate::step::light_response(sc, 1.0)
}

/// The adult's break-even cover at moisture `mu` in the open on a rich site.
fn adult_cover(sc: &SpeciesConfig, mu: f64) -> f64 {
    break_even_cover(sc, sc.wood_max, sc.alpha * sc.wood_max, l_open(sc), mu, monod(sc, RICH))
}

/// The available water whose ramp gives `mu`.
fn water_for(sc: &SpeciesConfig, mu: f64) -> f64 {
    sc.wilt_water + mu * (sc.full_water - sc.wilt_water)
}

/// The share of a stand's income left for upkeep once the reserve has taken its share.
fn usable(sc: &SpeciesConfig) -> f64 {
    1.0 - sc.reserve_share
}

/// Constant full sun at the reference temperature: no night, no hour.
fn sun(days: f64) -> Vec<ClimateSample> {
    let ticks = (days * 7200.0 / crate::DT) as usize;
    vec![ClimateSample { daylight: 1.0, upkeep: 1.0, night: 1.0, to_dawn_s: None }; ticks]
}

fn held(water: f64) -> Held {
    Held { water, saturated: 0.0, light: 1.0, mineral: RICH }
}

/// What a probe saw: the first tick cured, the first tick woke after it, the death time.
#[derive(Default, Debug)]
struct Seen {
    cured_at: Option<usize>,
    woke_at: Option<usize>,
    cured_foliage: f64,
    foliage_end: f64,
    died_s: Option<f64>,
}

fn run(c: &FloraConfig, climate: &[ClimateSample], water: impl Fn(usize) -> f64) -> Seen {
    let s0 = start_stand(c, TURF, Start::Adult);
    let mut seen = Seen::default();
    let mut was_cured = false;
    let o = probe_traced(c, s0, climate, |t| held(water(t)), |t, s: &Stand| {
        if s.cured && !was_cured && seen.cured_at.is_none() {
            seen.cured_at = Some(t);
            seen.cured_foliage = s.foliage;
        }
        if !s.cured && was_cured && seen.woke_at.is_none() {
            seen.woke_at = Some(t);
        }
        was_cured = s.cured;
        seen.foliage_end = s.foliage;
    });
    seen.died_s = o.died_s;
    seen
}

/// The moisture at which the adult's cover is `cover` (the law is linear in `μ`).
fn mu_at_cover(sc: &SpeciesConfig, cover: f64) -> f64 {
    let full = adult_cover(sc, 1.0);
    assert!(
        usable(sc) * full >= 1.5 * sc.cure_cover,
        "fixture: at full water the open adult pays its upkeep with room ({} · {full})",
        usable(sc)
    );
    let mu = cover / full;
    assert!(mu > 0.0 && mu < 1.0, "fixture: μ {mu} inside the ramp");
    mu
}

/// **The trait gap closed**: at a water between the stand's survival point and the old
/// cure point (inside the ramp, above `cure_water`, below `cure_resume_water`) where its
/// income pays only seven tenths of its upkeep and leaf turnover, the old rule (no budget
/// clock) starves it; the budget clock cures it, and it is still alive when the old rule's
/// stand died.
#[test]
fn a_stand_short_of_water_cures_and_lives_where_it_used_to_starve() {
    let c = band_config();
    let sc = &c.springturf;
    let water = water_for(sc, mu_at_cover(sc, 0.7));
    assert!(water >= sc.cure_water && water < sc.cure_resume_water, "fixture: inside the band, {water}");

    let mut old = c.clone();
    old.springturf.cure_cover = 0.0;
    let before = run(&old, &sun(6.0), |_| water);
    let died = before.died_s.expect("under the old rule the short stand starves");
    assert!(before.cured_at.is_none(), "the old rule never cures above cure_water");

    let ticks = (died / crate::DT) as usize + 1;
    let after = run(&c, &sun(6.0)[..ticks], |_| water);
    let cured = after.cured_at.expect("the budget clock cures the short stand");
    assert!(
        cured as f64 * crate::DT >= sc.cure_after_s - crate::DT,
        "it cures after cure_after_s short, not at once: {} s",
        cured as f64 * crate::DT
    );
    assert!(after.died_s.is_none(), "cured, it outlives the old rule's death at {died} s");
}

/// **It wakes at `cure_resume_water`** — not a hair below it — and regrows its foliage
/// from its reserve.
#[test]
fn a_cured_stand_wakes_at_resume_water_and_regrows_from_its_reserve() {
    let c = band_config();
    let sc = &c.springturf;
    let dry = water_for(sc, mu_at_cover(sc, 0.5));
    let wake = sc.cure_resume_water;
    let below = wake - 1e-6;
    // Short for twice the clock plus a little; then just below the wake water; then at it.
    let t_short = ((2.0 * sc.cure_after_s + 60.0) / crate::DT) as usize;
    let t_below = t_short + (600.0 / crate::DT) as usize;
    let climate = sun(1.0);
    let seen = run(&c, &climate, |t| if t < t_short { dry } else if t < t_below { below } else { wake });
    let cured = seen.cured_at.expect("cured while short");
    assert!(cured < t_short);
    let woke = seen.woke_at.expect("woke");
    assert_eq!(woke, t_below, "wakes on the first tick at cure_resume_water, and not below it");
    assert!(seen.died_s.is_none());
    assert!(
        seen.foliage_end > seen.cured_foliage,
        "regrew from its reserve: {} > {}",
        seen.foliage_end,
        seen.cured_foliage
    );
}

/// **Water that pays never cures**: inside the dry band but with income enough (after the
/// reserve's share) to pay a fifth over its upkeep, the stand stays green; at full water
/// likewise. And **shade is not drought**: a stand whose income could not pay even at full
/// water — a dim face — is not cured by the budget clock on a dry face; the clock holds.
#[test]
fn a_stand_whose_water_pays_never_cures_and_shade_is_not_drought() {
    let c = band_config();
    let sc = &c.springturf;
    let paying = water_for(sc, mu_at_cover(sc, 1.2 * sc.cure_cover / usable(sc)));
    assert!(paying < sc.cure_resume_water, "fixture: inside the band");
    for water in [paying, sc.full_water, 1.0] {
        let seen = run(&c, &sun(3.0), |_| water);
        assert!(seen.cured_at.is_none(), "water {water} pays: never cured");
        assert!(seen.died_s.is_none(), "water {water} pays: alive");
    }

    // The pure test on hand-built reads: dim light, a dry root box.
    let dim = CureRead {
        available: water_for(sc, 0.2),
        moisture: 0.2,
        light_eff: 0.0,
        stress: 0.0,
        mineral: RICH,
        wood: sc.wood_max,
        foliage: sc.alpha * sc.wood_max,
    };
    assert_eq!(water_short(sc, &dim), None, "no light: water is not what is short");
    let open = CureRead { light_eff: l_open(sc), ..dim };
    assert_eq!(water_short(sc, &open), Some(true), "open sky, dry: short on its water");
    let wet = CureRead { moisture: 1.0, available: sc.full_water, ..open };
    assert_eq!(water_short(sc, &wet), Some(false), "open sky, full water: pays");
}

/// **Config flow**: the budget clock's threshold is `cure_cover` on the usable income's
/// cover. A stand that pays at `cure_cover` is short once the knob asks for more than its
/// cover; zero turns the budget clock off.
#[test]
fn cure_cover_sets_where_the_budget_counts_short() {
    let mut sc = band_config().springturf.clone();
    let mu = 0.6;
    let r = CureRead {
        available: water_for(&sc, mu),
        moisture: mu,
        light_eff: l_open(&sc),
        stress: 0.0,
        mineral: RICH,
        wood: sc.wood_max,
        foliage: sc.alpha * sc.wood_max,
    };
    let cover = usable(&sc) * break_even_cover(&sc, r.wood, r.foliage, r.light_eff, mu, monod(&sc, RICH));
    let full = usable(&sc) * break_even_cover(&sc, r.wood, r.foliage, r.light_eff, 1.0, monod(&sc, RICH));
    assert!(full > cover * 1.2, "fixture: water limits");
    sc.cure_cover = 0.9 * cover;
    assert_eq!(water_short(&sc, &r), Some(false));
    sc.cure_cover = 1.1 * cover;
    assert_eq!(water_short(&sc, &r), Some(true));
    sc.cure_cover = 1.1 * full;
    assert_eq!(water_short(&sc, &r), None, "even full water would not pay this knob: not the water");
    sc.cure_cover = 0.0;
    assert_eq!(water_short(&sc, &r), Some(false), "off");
}

// ------------------------------------------------------------------- the world step

/// `width` soil columns at `available` water, one deep, 1 m voxels, open sky.
fn slab(width: u32, available: f64) -> World {
    let mut w = World::empty(VoxelConfig { width, height: 8, depth: 1, voxel_m: 1.0, seed: 7, ..VoxelConfig::default() });
    for x in 0..i64::from(width) {
        for y in 1..=2u32 {
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
    set_water(&mut w, available);
    w
}

fn pore_cap(w: &World) -> f64 {
    w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume()
}

/// Every soil voxel's pore water to `available` of the way from the wilting point to field
/// capacity.
fn set_water(w: &mut World, available: f64) {
    let m = Materials::DEFAULT;
    let (wp, fc) = (m.wilting_point(Material::Soil), m.field_capacity(Material::Soil));
    let cap = pore_cap(w);
    for x in 0..i64::from(w.config().width) {
        for y in 1..=2u32 {
            w.apply(WorldCommand::WithdrawPore { x, y, z: 0, volume_m3: cap });
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Air });
            w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: (wp + available * (fc - wp)) * cap });
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
}

fn face(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

fn assert_conserved(flora: &Flora, when: &str) {
    let v = flora.view();
    let o = v.organic() - v.ledger.expected_organic();
    let n = v.mineral() - v.ledger.expected_mineral();
    let e = v.energy() - v.ledger.expected_energy();
    assert!(o.abs() <= 1e-9 * v.organic().max(1.0), "{when}: organic {o}");
    assert!(n.abs() <= 1e-9 * v.mineral().max(1.0), "{when}: mineral {n}");
    assert!(e.abs() <= 1e-9 * v.energy().max(1.0), "{when}: energy {e}");
}

/// **The ledgers close through a budget cure and a wake**: a lone turf on a slab whose
/// root box sits at a fifth of its ramp, the absolute clock off (`cure_water` 0) so only
/// the budget can cure it; then the slab is wetted to field capacity and it wakes. Every
/// tick conserves organic matter, mineral and energy; the ledger counts one cure and one
/// wake, and the cure's shed foliage is in the litter.
#[test]
fn the_ledgers_close_through_a_budget_cure_and_a_wake() {
    let mut c = FloraConfig::default();
    for sp in Species::ALL {
        let s = c.species_mut(sp);
        s.seed_attrition_per_s = 0.0;
        s.propagule_rate = 0.0;
        s.burr_pickup = 0.0;
    }
    let t = &mut c.springturf;
    t.wilt_water = 0.0;
    t.full_water = 1.0;
    t.cure_water = 0.0;
    t.cure_resume_water = 0.6;
    t.cure_after_s = 1.0;
    t.cure_max_s = 0.0;
    let mut world = slab(3, 0.2);
    let mut flora = Flora::new(c.clone());
    let wood = c.springturf.wood_max;
    assert!(flora.apply(&world, Command::SeedOnFace { site: face(1), species: TURF, wood }));
    let turf = |f: &Flora| f.view().stand_at(face(1)).copied();
    let mut cured = None;
    for _ in 0..2000 {
        let litter0 = flora.view().ground_at(face(1)).map_or(0.0, |g| g.litter);
        flora.step(&mut world);
        assert_conserved(&flora, "short");
        let s = turf(&flora).expect("alive while short");
        if s.cured {
            cured = Some(litter0);
            break;
        }
    }
    let litter0 = cured.expect("the budget clock cured it");
    let l = flora.view().ledger;
    assert_eq!(l.cures[TURF.index()], 1);
    let shed = l.cure_shed_organic[TURF.index()];
    assert!(shed > 0.0);
    let litter = flora.view().ground_at(face(1)).map_or(0.0, |g| g.litter);
    assert!(litter >= litter0 + 0.99 * shed, "the cure's foliage is in the litter");
    for _ in 0..50 {
        flora.step(&mut world);
        assert_conserved(&flora, "dormant");
    }
    set_water(&mut world, 1.0);
    for _ in 0..200 {
        flora.step(&mut world);
        assert_conserved(&flora, "woken");
    }
    let l = flora.view().ledger;
    assert_eq!(l.cure_wakes[TURF.index()], 1, "woke once");
    let s = turf(&flora).expect("alive");
    assert!(!s.cured);
    assert_eq!(l.deaths_by_cause[DeathCause::Starved.index()][TURF.index()], 0);
}

/// **A dormant stand whose stores run out dies of drought**, not of starvation: dormant on
/// its full upkeep (`cure_upkeep` 1) at a raised maintenance so the stores go in minutes,
/// cured by the absolute clock on a slab below `cure_water`.
#[test]
fn a_dormant_stand_whose_stores_run_out_dies_of_drought() {
    let mut c = FloraConfig::default();
    for sp in Species::ALL {
        let s = c.species_mut(sp);
        s.seed_attrition_per_s = 0.0;
        s.propagule_rate = 0.0;
        s.burr_pickup = 0.0;
    }
    let t = &mut c.springturf;
    t.cure_water = 0.5;
    t.cure_resume_water = 0.9;
    t.cure_after_s = 1.0;
    t.cure_upkeep = 1.0;
    t.cure_max_s = 0.0;
    t.maintenance *= 200.0;
    let mut world = slab(3, 0.1);
    let mut flora = Flora::new(c.clone());
    let wood = c.springturf.wood_max;
    assert!(flora.apply(&world, Command::SeedOnFace { site: face(1), species: TURF, wood }));
    let mut cured = false;
    for _ in 0..20_000 {
        flora.step(&mut world);
        assert_conserved(&flora, "drying");
        match flora.view().stand_at(face(1)) {
            Some(s) => cured |= s.cured,
            None => break,
        }
    }
    assert!(cured, "it cured first");
    assert!(flora.view().stand_at(face(1)).is_none(), "the stores ran out");
    let l = flora.view().ledger;
    assert_eq!(l.deaths_by_cause[DeathCause::Drought.index()][TURF.index()], 1, "a dormant death is a drought death");
    assert_eq!(l.deaths_by_cause[DeathCause::Starved.index()][TURF.index()], 0);
}
