//! The seed bank and dispersal: package S of `design/handoffs/voxel-plant-viability-2026-09-23.md`,
//! steps 8 and 9 of the tick (`step.rs`'s module doc).
//!
//! # Whole seeds
//!
//! A bank holds **whole seeds**. A seed is one package — `alive_min / w_frac`, the
//! material a germination needs to build a stand at exactly `alive_min` of wood — and a
//! [`SeedCohort`] (one species, one arrival bin) holds a whole number of them: every
//! landing adds one package, attrition kills whole seeds and germination spends one.
//! The count is `organic / package`, rounded ([`seeds_in`]); the cohort keeps its organic
//! matter and mineral as sums, and a seed taken out of it takes `1 / n` of each.
//!
//! Before this a bank was a continuous amount: attrition took a fraction of it every tick,
//! so a lone package fell under one package the tick after it landed and could never
//! germinate, and a recruit needed two landings. A lone seed now waits for its gap.
//!
//! # The check wheel
//!
//! A banked site is tested — germination, then attrition and age — only on its own check,
//! once every [`FloraConfig::seed_check_s`] at a phase hashed from the site ([`check_phase`]), and
//! on the tick a shower ends ([`Flora::was_raining`] falling), when every bank is tested for
//! germination at once: the moisture cue. So the bank costs
//! `banked sites / period` site visits per tick, not a scan of every site, and
//! a bank that lasts hours is not a bank that costs hours of ticks.
//!
//! The wheel ([`Flora::bank_wheel`]) is derived state: one bucket of sites per phase,
//! rebuilt from the ground whenever it is missing (a decoded snapshot, a fixture). A site
//! joins its bucket when its bank goes from empty to banked; a site whose bank has gone —
//! spent, cleared, pruned — leaves at its next check.
//!
//! # Dispersal
//!
//! A donor's whole package is a **runner** (a daughter stand on a free neighbouring face
//! that passes the gates, with probability [`SpeciesConfig::clonal_share`]) or **one
//! seed**, landed by the species' [`Dispersal`] mode ([`landing`]).

use cubarium_voxel::{DT, VoxelView, World};

use crate::step::{
    DOMAIN_ATTRITION, DOMAIN_CLONAL, DOMAIN_DISPERSAL, DOMAIN_GERMINATION, DOMAIN_RUNNER,
    DOMAIN_SPORES, DOMAIN_WATER, DOMAIN_WIND, Rng, add_litter_cap, establishes, gates, mix,
    provisioned_slot, pull_mineral, standing_water_beside, substrate_pools_in_box,
};
use crate::{
    DeliveryReceipt, Dispersal, Flora, FloraConfig, FloraLedger, Gates, Ground, SeedCohort, Site,
    Species, SpeciesConfig, Stage, Stand, highest_support,
};

/// How many ticks apart a banked site's checks are: [`FloraConfig::seed_check_s`] in
/// whole ticks, at least one.
pub(crate) fn check_period(config: &FloraConfig) -> u64 {
    let ticks = (config.seed_check_s / DT).round();
    if ticks.is_finite() && ticks >= 1.0 {
        ticks as u64
    } else {
        1
    }
}

/// A spore package's tries at a site whose ground passes: **8**. The rain that finds none
/// in eight falls as litter on the last site it tried — the spore rain that mostly falls
/// where nothing grows. Eight bounds the cost to eight gate readings a package, and loses
/// under a tenth of the rain where a quarter of the ground passes (`0.75⁸ = 0.10`).
const SPORE_TRIES: usize = 8;

/// A wind seed's tries at a column with a support face before the donor keeps its package
/// for the next tick: **4**. Only a void column refuses one.
const WIND_TRIES: usize = 4;

/// The wind kernel's distance is a Lomax (Pareto II) draw, `P(R > r) = (1 + r/a)^-k`,
/// with shape `k` **2** and scale `a` **2.5 hops**: the median seed lands 1.04 hops out,
/// 79 % within 3 hops, and 4 % past 10 hops — the brief's "most within a few hops, a few
/// percent past 10". Shape 2 is the thinnest tail that still has a few percent that far
/// out at a median near one hop.
const WIND_SHAPE: f64 = 2.0;
const WIND_SCALE_HOPS: f64 = 2.5;

/// The smallest package worth sending, and **one seed**: the seed-bank organic matter a
/// germination needs to build a stand at exactly `alive_min` of wood, `alive_min / w_frac`
/// — 0.05 at the base. Stated as a division guarded by the caller, never a tuned constant.
///
/// Zero for a species whose `w_frac` is zero, which can never germinate anything.
pub(crate) fn package_of(sc: &SpeciesConfig) -> f64 {
    let w_frac = sc.propagule_split[0];
    if w_frac > 0.0 {
        sc.alive_min / w_frac
    } else {
        0.0
    }
}

/// How many whole seeds a cohort holds: `organic / package`, rounded, so the ulps a sum
/// of packages collects never cost a seed. Zero for a species with no package.
pub(crate) fn seeds_in(c: &SeedCohort, package: f64) -> u64 {
    if package > 0.0 && c.organic > 0.0 {
        (c.organic / package).round() as u64
    } else {
        0
    }
}

/// The three stocks a funded newborn is built out of `organic` units of material: wood at
/// **exactly** `alive_min`, foliage at the preset's `p_frac` of the material, and the
/// remainder — including every unit of floating-point difference — in the reserve.
///
/// Astra's R5.1: a newborn whose wood was `w_frac · organic` could land an ulp under
/// `alive_min` and die on its first growth tick having been paid for in full. The wood is
/// the threshold itself, and the rounding difference goes to the reserve, which feeds no
/// income and no death test. `wood + foliage + reserve` re-sums to `organic`, and every
/// stock is non-negative by construction.
pub(crate) fn newborn_stocks(sc: &SpeciesConfig, organic: f64) -> (f64, f64, f64) {
    let wood = sc.alive_min.min(organic.max(0.0));
    let left = (organic - wood).max(0.0);
    let foliage = (sc.propagule_split[1] * organic).clamp(0.0, left);
    (wood, foliage, left - foliage)
}

/// Which species takes a gap, among the banks that can build a stand on it: a draw
/// weighted by the **whole seeds** each holds, from a stream keyed by the world's seed, the
/// site's voxel index and the tick. `None` when nothing qualifies.
///
/// Astra's R4.5: a fixed enum order is not an ecological rule. The candidates are sorted
/// by species before the walk, so the caller's order cannot select the winner, and the
/// weights are integers, so there is no float comparison in the draw.
pub(crate) fn lottery(
    world_seed: u64,
    site_index: u64,
    tick: u64,
    candidates: &[(Species, u64)],
) -> Option<Species> {
    match candidates {
        [] => return None,
        [(one, _)] => return Some(*one),
        _ => {}
    }
    let mut sorted: Vec<(Species, u64)> = candidates.to_vec();
    sorted.sort_unstable_by_key(|&(species, _)| species);
    let total: u64 = sorted.iter().map(|&(_, w)| w).sum();
    if total == 0 {
        return None;
    }
    let mut draw =
        Rng::keyed(DOMAIN_GERMINATION, world_seed, site_index, tick).below(total as usize) as u64;
    for (species, weight) in sorted {
        if draw < weight {
            return Some(species);
        }
        draw -= weight;
    }
    None
}

/// Spend **one seed** of `species` from the oldest bin that holds one: `1 / n` of that
/// bin's organic matter and mineral, or the whole bin when it is the last seed in it, so
/// no float dust is left behind claiming to be a cohort. `None` when the bank holds no
/// whole seed of the species.
pub(crate) fn spend_seed(g: &mut Ground, species: Species, package: f64) -> Option<(f64, f64)> {
    let i = g
        .seeds
        .iter()
        .position(|c| c.species == species && seeds_in(c, package) >= 1)?;
    let n = seeds_in(&g.seeds[i], package);
    if n == 1 {
        let c = g.seeds.remove(i);
        return Some((c.organic, c.mineral));
    }
    let c = &mut g.seeds[i];
    let organic = c.organic / n as f64;
    let mineral = c.mineral / n as f64;
    c.organic -= organic;
    c.mineral -= mineral;
    Some((organic, mineral))
}

/// The width of one arrival bin in ticks: `seed_max_age_s / seed_cohorts_max`, **rounded
/// up**, at least one tick. Derived, not a knob of its own. Rounded up so that the bound
/// on the bank is exact: a lifetime of `life` ticks in bins of `ceil(life / n)` holds at
/// most `n + 1` live bins.
pub(crate) fn bin_ticks(sc: &SpeciesConfig) -> u64 {
    let ticks = sc.seed_max_age_s / sc.seed_cohorts_max.max(1) as f64 / DT;
    if ticks.is_finite() && ticks >= 1.0 {
        ticks.ceil() as u64
    } else {
        1
    }
}

/// The first tick of the bin that `tick` falls in.
pub(crate) fn bin_start(tick: u64, sc: &SpeciesConfig) -> u64 {
    let w = bin_ticks(sc);
    tick - tick % w
}

/// Land material on a site's bank, in the bin whose window covers `tick`: a bin already
/// there sums the organic matter and the mineral and keeps its start, and one that is not
/// is inserted. `seeds` stays sorted by species then `bin_start_tick`, oldest first.
fn add_cohort(
    g: &mut Ground,
    species: Species,
    organic: f64,
    mineral: f64,
    tick: u64,
    sc: &SpeciesConfig,
) {
    if organic <= 0.0 && mineral <= 0.0 {
        return;
    }
    let start = bin_start(tick, sc);
    match g
        .seeds
        .binary_search_by(|c| (c.species, c.bin_start_tick).cmp(&(species, start)))
    {
        Ok(i) => {
            g.seeds[i].organic += organic;
            g.seeds[i].mineral += mineral;
        }
        Err(i) => g.seeds.insert(
            i,
            SeedCohort {
                species,
                organic,
                mineral,
                bin_start_tick: start,
            },
        ),
    }
}

// ------------------------------------------------------------------- the check wheel

/// The phase of `site`'s check within a period of `period` ticks: a hash of the site, so
/// the checks of neighbouring sites are spread over the period rather than bunched on one
/// tick, and the same site is always checked on the same phase.
pub(crate) fn check_phase(site: Site, period: u64) -> u64 {
    let key = (u64::from(site.x) << 42) ^ (u64::from(site.y) << 21) ^ u64::from(site.z);
    mix(key ^ 0x5EED_BA4C_C4EC_4000) % period.max(1)
}

/// Rebuild the wheel from the ground if it is missing or its period changed.
fn ensure_wheel(flora: &mut Flora) {
    let period = check_period(&flora.config);
    if flora.bank_wheel.len() == period as usize {
        return;
    }
    let mut wheel = vec![Vec::new(); period as usize];
    for g in &flora.ground {
        if !g.seeds.is_empty() {
            wheel[check_phase(g.site, period) as usize].push(g.site);
        }
    }
    flora.bank_wheel = wheel;
}

/// Put `site` on the wheel, for a fixture that banks seeds by hand. A site already on it
/// is deduplicated at its check.
#[cfg(test)]
pub(crate) fn schedule(flora: &mut Flora, site: Site) {
    ensure_wheel(flora);
    let period = check_period(&flora.config);
    flora.bank_wheel[check_phase(site, period) as usize].push(site);
}

// ------------------------------------------------------------------- 8. the seed bank

/// Step 8: the sites due this tick — and on a shower's last tick every banked site — hold
/// their germination lottery; then the due sites alone pay attrition and age.
///
/// Germination first, as before (`tests/round3.rs`'s expiry boundary): a bin past its
/// `seed_max_age_s` at a check gets that check's lottery before it goes to litter.
pub(crate) fn seed_bank(flora: &mut Flora, world: &World) {
    ensure_wheel(flora);
    let view = world.view();
    let raining = view.is_raining();
    let flush = flora.was_raining && !raining;
    flora.was_raining = raining;

    let Flora {
        config,
        tick,
        stands,
        ground,
        ledger,
        sky,
        bank_wheel,
        ..
    } = flora;
    let tick = *tick;
    let world_seed = view.config.seed;
    let period = check_period(config);
    let slot = (tick % period) as usize;

    // The due sites, as ground indices in site order. A site whose bank has gone leaves
    // the wheel here.
    let mut due_sites = std::mem::take(&mut bank_wheel[slot]);
    due_sites.sort_unstable();
    due_sites.dedup();
    let mut due: Vec<usize> = Vec::with_capacity(due_sites.len());
    for site in due_sites {
        if let Ok(gi) = ground.binary_search_by_key(&site, |g| g.site)
            && !ground[gi].seeds.is_empty()
        {
            due.push(gi);
        }
    }

    if flush {
        let every: Vec<usize> = (0..ground.len())
            .filter(|&gi| !ground[gi].seeds.is_empty())
            .collect();
        germinate(
            config, tick, world_seed, &view, stands, ground, ledger, sky, &every,
        );
    } else {
        germinate(
            config, tick, world_seed, &view, stands, ground, ledger, sky, &due,
        );
    }
    for &gi in &due {
        age_bank(
            config,
            &view,
            world_seed,
            tick,
            period,
            &mut ground[gi],
            ledger,
        );
    }
    let bucket = &mut bank_wheel[slot];
    for &gi in &due {
        if !ground[gi].seeds.is_empty() {
            bucket.push(ground[gi].site);
        }
    }
}

/// The germination lottery on each tested site that has no stand: among the species with
/// at least one whole seed there that pass the predicate, weighted by seeds; the winner
/// spends one. Drawn in a read-only pass first — a saprotroph's gate reads other sites'
/// dead wood — and spent after.
#[allow(clippy::too_many_arguments)]
fn germinate(
    config: &FloraConfig,
    tick: u64,
    world_seed: u64,
    view: &VoxelView<'_>,
    stands: &mut Vec<Stand>,
    ground: &mut [Ground],
    ledger: &mut FloraLedger,
    sky: &mut Vec<(Site, f64)>,
    tested: &[usize],
) {
    let mut winners: Vec<(usize, Species)> = Vec::new();
    for &gi in tested {
        let site = ground[gi].site;
        if stands.binary_search_by_key(&site, |s| s.site).is_ok() {
            continue;
        }
        let mut counts = [0u64; Species::COUNT];
        for c in &ground[gi].seeds {
            counts[c.species.index()] += seeds_in(c, package_of(config.species(c.species)));
        }
        let mut candidates: Vec<(Species, u64)> = Vec::new();
        for species in Species::ALL {
            let n = counts[species.index()];
            if n == 0 {
                continue;
            }
            if !establishes(view, sky, ground, site, config.species(species)) {
                continue;
            }
            candidates.push((species, n));
        }
        let site_index = view.config.index(site.x as i64, site.y, site.z) as u64;
        if let Some(species) = lottery(world_seed, site_index, tick, &candidates) {
            winners.push((gi, species));
        }
    }
    for (gi, species) in winners {
        let g = &mut ground[gi];
        let sc = config.species(species);
        // The slot first and the bank second: a seed is never spent on a birth that does
        // not happen.
        let at = match stands.binary_search_by_key(&g.site, |s| s.site) {
            Ok(_) => continue,
            Err(at) => at,
        };
        let Some((organic, mineral)) = spend_seed(g, species, package_of(sc)) else {
            continue;
        };
        stands.insert(
            at,
            newborn(config, ledger, g.site, species, organic, mineral),
        );
        ledger.establishments += 1;
        ledger.seeds_germinated[species.index()] += 1;
    }
}

/// A stand built out of one funded package: the seed that germinated or the runner that
/// rooted. Its id is the next birth.
fn newborn(
    config: &FloraConfig,
    ledger: &mut FloraLedger,
    site: Site,
    species: Species,
    organic: f64,
    mineral: f64,
) -> Stand {
    let sc = config.species(species);
    let (wood, foliage, reserve) = newborn_stocks(sc, organic);
    let id = ledger.births;
    ledger.births += 1;
    let mut born = Stand {
        id,
        site,
        species,
        stage: Stage::Alive,
        wood,
        foliage,
        reserve,
        light: 0.0,
        moisture: 0.0,
        water_m3: 0.0,
        mineral,
        aeration_stress: 0.0,
        parcel: 0.0,
        layer_stock: [0.0; crate::MAX_FOLIAGE_LAYERS],
        profile_stage: 0,
    };
    // A newborn's foliage goes into its seedling profile bottom-up, like any other tissue.
    born.bin_foliage(sc, config.voxel_m);
    born
}

/// One check's decay for one site's bank. A bin whose **start** is past `seed_max_age_s`
/// falls to litter whole, and so does dust too small to be a seed. Every other bin loses
/// **whole seeds**: each survives the time since the site's last check —
/// `min(period, age)`, so a bin opened since then pays only for its own age —
/// with probability `exp(−seed_attrition_per_s · t)`, one keyed draw per seed. A dead
/// seed's `1 / n` of the bin's organic matter and mineral goes to litter with its energy:
/// paid decay, never deletion.
///
/// A seed that landed partway through the period pays for the whole of it: at most one
/// period's attrition (under 0.5 % at every species' rate) and the price of not touching
/// the site between checks.
fn age_bank(
    config: &FloraConfig,
    view: &VoxelView<'_>,
    world_seed: u64,
    tick: u64,
    period: u64,
    g: &mut Ground,
    ledger: &mut FloraLedger,
) {
    if g.seeds.is_empty() {
        return;
    }
    let cap = config.litter_energy_cap;
    let site_index = view.config.index(g.site.x as i64, g.site.y, g.site.z) as u64;
    let mut rng = Rng::keyed(DOMAIN_ATTRITION, world_seed, site_index, tick);
    let taken = std::mem::take(&mut g.seeds);
    let mut kept: Vec<SeedCohort> = Vec::with_capacity(taken.len());
    for mut c in taken {
        let sc = config.species(c.species);
        let slot = c.species.index();
        let e_v = sc.energy_density;
        let n = seeds_in(&c, package_of(sc));
        if n == 0 || c.age_s(tick) > sc.seed_max_age_s {
            add_litter_cap(cap, g, c.organic, c.mineral, e_v * c.organic, ledger);
            ledger.seeds_died[slot] += n;
            continue;
        }
        let elapsed_s = c.age_ticks(tick).min(period) as f64 * DT;
        let p = 1.0 - (-sc.seed_attrition_per_s * elapsed_s).exp();
        let mut dead = 0u64;
        if p > 0.0 {
            for _ in 0..n {
                if rng.unit() < p {
                    dead += 1;
                }
            }
        }
        if dead == n {
            add_litter_cap(cap, g, c.organic, c.mineral, e_v * c.organic, ledger);
            ledger.seeds_died[slot] += n;
            continue;
        }
        if dead > 0 {
            let f = dead as f64 / n as f64;
            let (organic, mineral) = (c.organic * f, c.mineral * f);
            c.organic -= organic;
            c.mineral -= mineral;
            add_litter_cap(cap, g, organic, mineral, e_v * organic, ledger);
            ledger.seeds_died[slot] += dead;
        }
        kept.push(c);
    }
    g.seeds = kept;
}

// ------------------------------------------------------------------- 9. propagules

/// A runner daughter paid for this tick, built after the sweep so no index moves under it.
struct Daughter {
    site: Site,
    species: Species,
    organic: f64,
    mineral: f64,
    donor: u64,
}

/// §4.8, round 3b's saving rule, and package S's two ways a package leaves.
///
/// Every tick, a stand over `donor_min` asks for `propagule_rate · dt`, gross, funded out
/// of its reserve above `donor_reserve_floor`; what it pays is respired for construction
/// (`c_g`) and the rest saved in [`Stand::parcel`] (Astra R4.4). When the parcel holds one
/// whole package it leaves:
///
/// - with probability [`SpeciesConfig::clonal_share`], as a **runner**: a daughter stand on
///   a free neighbouring face that passes the species' gates, built from the package
///   exactly as a germination builds one. Nothing is banked. A runner with nowhere to go
///   leaves as a seed instead, so nothing paid for is stranded;
/// - otherwise as **one seed**, landed by the species' [`Dispersal`] mode ([`landing`]):
///   into the bank of the site it lands on, or — a spore or water seed that finds no
///   ground — into the litter where it fell.
///
/// A mode that finds no site at all this tick (a void column under a wind seed) leaves
/// the package in the parcel for the next tick.
pub(crate) fn propagate(flora: &mut Flora, world: &World) {
    ensure_wheel(flora);
    let Flora {
        config,
        tick,
        stands,
        ground,
        ledger,
        deliveries,
        sky,
        bank_wheel,
        ..
    } = flora;
    let tick = *tick;
    let view = world.view();
    let world_seed = view.config.seed;
    let period = check_period(config);
    let mut daughters: Vec<Daughter> = Vec::new();

    for si in 0..stands.len() {
        let donor = stands[si];
        let sc = config.species(donor.species);
        let e_v = sc.energy_density;
        let slot = donor.species.index();
        let build = 1.0 + sc.build;

        // ---- what the rate asks for, and what the reserve can fund.
        if donor.wood >= sc.donor_min {
            let ask = (sc.propagule_rate * DT).max(0.0);
            ledger.propagule_requested[slot] += ask / build;
            let floor = sc.donor_reserve_floor * sc.reserve_cap * donor.wood;
            let take = (donor.reserve - floor)
                .max(0.0)
                .min(ask)
                .min(donor.reserve.max(0.0));
            if take > 0.0 {
                let net = take / build;
                stands[si].reserve -= take;
                stands[si].parcel += net;
                ledger.propagule_funded[slot] += net;
                // Construction respiration, paid when the material is set aside; the
                // parcel's mineral stays in the stand until the package leaves.
                ledger.respired_out += take - net;
                ledger.heat_out += e_v * (take - net);
            }
        }

        // ---- one whole package or nothing. A stand below `donor_min` still delivers
        // material it has already paid for.
        let package = package_of(sc);
        if package <= 0.0 || stands[si].parcel < package {
            continue;
        }
        let home = donor.site;
        let home_index = view.config.index(home.x as i64, home.y, home.z) as u64;

        // ---- a runner, when the draw says so and a neighbour will take it.
        if sc.clonal_share > 0.0
            && Rng::keyed(DOMAIN_CLONAL, world_seed, home_index, tick).unit() < sc.clonal_share
            && let Some(site) = runner_target(
                &view, sky, ground, stands, &daughters, home, sc, world_seed, tick,
            )
        {
            let before = stands[si].material();
            stands[si].parcel -= package;
            let mineral = pull_mineral(&mut stands[si], before, package);
            ledger.propagule_landed[slot] += package;
            daughters.push(Daughter {
                site,
                species: donor.species,
                organic: package,
                mineral,
                donor: donor.id,
            });
            continue;
        }

        // ---- one seed.
        let (site, banked) = match landing(&view, ground, sc, home, world_seed, tick) {
            Landing::Bank(site) => (site, true),
            Landing::Lost(site) => (site, false),
            Landing::Retry => continue,
        };
        let before = stands[si].material();
        stands[si].parcel -= package;
        // The package takes the same fraction of the donor's mineral as it is of the
        // donor's whole material, parcel included.
        let mineral = pull_mineral(&mut stands[si], before, package);
        ledger.propagule_landed[slot] += package;
        let gi = provisioned_slot(config, ground, site, ledger);
        if banked {
            if ground[gi].seeds.is_empty() {
                bank_wheel[check_phase(site, period) as usize].push(site);
            }
            add_cohort(&mut ground[gi], donor.species, package, mineral, tick, sc);
            ledger.seeds_landed[slot] += 1;
            // The receipt, written where the destination is known (Astra R10.3).
            deliveries.push(DeliveryReceipt {
                tick,
                donor: donor.id,
                species: donor.species,
                recipient: site,
                organic: package,
                mineral,
            });
        } else {
            add_litter_cap(
                config.litter_energy_cap,
                &mut ground[gi],
                package,
                mineral,
                e_v * package,
                ledger,
            );
            ledger.seeds_lost[slot] += 1;
        }
    }

    // ---- the daughters, now that the sweep is over.
    for d in daughters {
        let slot = d.species.index();
        let gi = provisioned_slot(config, ground, d.site, ledger);
        let at = match stands.binary_search_by_key(&d.site, |s| s.site) {
            Ok(_) => {
                // Unreachable — `runner_target` skips occupied and claimed faces — and
                // harmless if that changes: the paid package is banked where it fell.
                if ground[gi].seeds.is_empty() {
                    bank_wheel[check_phase(d.site, period) as usize].push(d.site);
                }
                add_cohort(
                    &mut ground[gi],
                    d.species,
                    d.organic,
                    d.mineral,
                    tick,
                    config.species(d.species),
                );
                ledger.seeds_landed[slot] += 1;
                continue;
            }
            Err(at) => at,
        };
        stands.insert(
            at,
            newborn(config, ledger, d.site, d.species, d.organic, d.mineral),
        );
        ledger.establishments += 1;
        ledger.clonal_births[slot] += 1;
        deliveries.push(DeliveryReceipt {
            tick,
            donor: d.donor,
            species: d.species,
            recipient: d.site,
            organic: d.organic,
            mineral: d.mineral,
        });
    }
}

/// The face a runner roots on: one of the eight neighbouring columns' support faces
/// within one voxel of the parent's height (a runner crawls; it does not climb a cliff or
/// drop off one), with no stand on it, not claimed by another runner this tick, and
/// passing the species' predicate. Drawn uniformly from the parent's own stream; `None`
/// when there is no such face.
#[allow(clippy::too_many_arguments)]
fn runner_target(
    view: &VoxelView<'_>,
    sky: &mut Vec<(Site, f64)>,
    ground: &[Ground],
    stands: &[Stand],
    claimed: &[Daughter],
    home: Site,
    sc: &SpeciesConfig,
    world_seed: u64,
    tick: u64,
) -> Option<Site> {
    let c = view.config;
    let mut free: Vec<Site> = Vec::new();
    for dz in -1i64..=1 {
        let z = home.z as i64 + dz;
        if z < 0 || z >= c.depth as i64 {
            continue;
        }
        for dx in -1i64..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            let x = home.x as i64 + dx;
            let Some(site) = support_near(view, x, z as u32, home.y) else {
                continue;
            };
            if site == home
                || free.contains(&site)
                || stands.binary_search_by_key(&site, |s| s.site).is_ok()
                || claimed.iter().any(|d| d.site == site)
                || !establishes(view, sky, ground, site, sc)
            {
                continue;
            }
            free.push(site);
        }
    }
    if free.is_empty() {
        return None;
    }
    let home_index = c.index(home.x as i64, home.y, home.z) as u64;
    Some(free[Rng::keyed(DOMAIN_RUNNER, world_seed, home_index, tick).below(free.len())])
}

/// The support face in column `(x, z)` at `y`, or one voxel above or below it.
fn support_near(view: &VoxelView<'_>, x: i64, z: u32, y: u32) -> Option<Site> {
    let width = view.config.width as i64;
    [Some(y), y.checked_add(1), y.checked_sub(1)]
        .into_iter()
        .flatten()
        .find(|&yy| view.is_support(x, yy, z))
        .map(|yy| Site {
            x: x.rem_euclid(width) as u32,
            y: yy,
            z,
        })
}

/// Where one seed goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Landing {
    /// Into the bank of this site.
    Bank(Site),
    /// Onto this site as litter: a spore or water seed that found no ground.
    Lost(Site),
    /// Nowhere this tick; the donor keeps the package.
    Retry,
}

/// Where one seed from a donor on `home` lands, by the species' [`Dispersal`] mode. Every
/// draw is keyed by the world's seed, the donor's site and the tick.
pub(crate) fn landing(
    view: &VoxelView<'_>,
    ground: &[Ground],
    sc: &SpeciesConfig,
    home: Site,
    world_seed: u64,
    tick: u64,
) -> Landing {
    let home_index = view.config.index(home.x as i64, home.y, home.z) as u64;
    match sc.dispersal {
        Dispersal::Drop => drop_target(view, sc.hop, home, world_seed, home_index, tick)
            .map_or(Landing::Retry, Landing::Bank),
        Dispersal::Wind => wind_target(view, sc.hop, home, world_seed, home_index, tick)
            .map_or(Landing::Retry, Landing::Bank),
        Dispersal::Spores => spore_target(view, ground, sc, home, world_seed, home_index, tick),
        Dispersal::Water => water_target(view, sc, home, world_seed, home_index, tick),
    }
}

/// **Drop**: the highest support face of each column within `hop` in `x` and `z` is a
/// candidate — one per column — and one is drawn uniformly. Never the donor's own site,
/// no habitat screening: the bank waits for the gap. Unchanged from round 3b.
fn drop_target(
    view: &VoxelView<'_>,
    hop: u32,
    home: Site,
    world_seed: u64,
    home_index: u64,
    tick: u64,
) -> Option<Site> {
    let mut targets: Vec<Site> = Vec::new();
    let hop = hop as i64;
    for dz in -hop..=hop {
        let z = home.z as i64 + dz;
        if z < 0 || z >= view.config.depth as i64 {
            continue;
        }
        for dx in -hop..=hop {
            let Some(site) = highest_support(view, home.x as i64 + dx, z as u32) else {
                continue;
            };
            if site == home || targets.contains(&site) {
                continue;
            }
            targets.push(site);
        }
    }
    if targets.is_empty() {
        return None;
    }
    let mut rng = Rng::keyed(DOMAIN_DISPERSAL, world_seed, home_index, tick);
    Some(targets[rng.below(targets.len())])
}

/// One wind offset in whole columns: a Lomax distance ([`WIND_SHAPE`], [`WIND_SCALE_HOPS`]
/// hops), capped at `cap` columns, in a uniform direction. A seed that would fall back on
/// its own column lands one column out in the same direction, so the offset is never
/// `(0, 0)`.
pub(crate) fn wind_offset(rng: &mut Rng, hop: u32, cap: f64) -> (i64, i64) {
    let a = WIND_SCALE_HOPS * f64::from(hop.max(1));
    let u = rng.unit();
    let r = (a * ((1.0 - u).powf(-1.0 / WIND_SHAPE) - 1.0)).min(cap);
    let theta = std::f64::consts::TAU * rng.unit();
    let (s, co) = theta.sin_cos();
    let (dx, dz) = ((r * co).round() as i64, (r * s).round() as i64);
    if (dx, dz) == (0, 0) {
        (co.round() as i64, s.round() as i64)
    } else {
        (dx, dz)
    }
}

/// `z` folded back into `0..depth` off the two walls: a seed blown past the edge of the
/// world bounces off the glass rather than piling up against it.
fn reflect(z: i64, depth: u32) -> u32 {
    let d = i64::from(depth);
    if d <= 1 {
        return 0;
    }
    let period = 2 * (d - 1);
    let m = z.rem_euclid(period);
    (if m < d { m } else { period - m }) as u32
}

/// **Wind**: a fat-tailed, isotropic distance ([`wind_offset`], capped at the world's
/// larger side), wrapped round the ring in `x` and reflected off the walls in `z`, onto
/// the column's highest support face. No habitat screening, like a drop.
fn wind_target(
    view: &VoxelView<'_>,
    hop: u32,
    home: Site,
    world_seed: u64,
    home_index: u64,
    tick: u64,
) -> Option<Site> {
    let c = view.config;
    let cap = f64::from(c.width.max(c.depth));
    let mut rng = Rng::keyed(DOMAIN_WIND, world_seed, home_index, tick);
    for _ in 0..WIND_TRIES {
        let (dx, dz) = wind_offset(&mut rng, hop, cap);
        let z = reflect(home.z as i64 + dz, c.depth);
        if let Some(site) = highest_support(view, home.x as i64 + dx, z)
            && site != home
        {
            return Some(site);
        }
    }
    None
}

/// Every gate but light: the ground a spore can wait on. Light is the bank's to wait for
/// — a spore rain falls in the shade too. Written as the predicate with the light gate
/// opened, so a gate added to [`Gates`] is read here without a second list of them.
fn ground_passes(view: &VoxelView<'_>, ground: &[Ground], site: Site, sc: &SpeciesConfig) -> bool {
    let (dead_wood, litter) = substrate_pools_in_box(view, ground, site, sc);
    let g = gates(view, site, sc, 1.0, dead_wood, litter);
    Gates {
        light_ok: true,
        ..g
    }
    .passes()
}

/// **Spores**: up to [`SPORE_TRIES`] columns drawn uniformly within `hop` — a spore
/// species' `hop` is its wide radius — and the spore lands on the first whose highest
/// support face passes the species' ground gates ([`ground_passes`]). The rain that finds
/// none is litter on the last face it tried.
fn spore_target(
    view: &VoxelView<'_>,
    ground: &[Ground],
    sc: &SpeciesConfig,
    home: Site,
    world_seed: u64,
    home_index: u64,
    tick: u64,
) -> Landing {
    let c = view.config;
    let hop = sc.hop as i64;
    let span = (2 * hop + 1) as usize;
    let mut rng = Rng::keyed(DOMAIN_SPORES, world_seed, home_index, tick);
    let mut last = None;
    for _ in 0..SPORE_TRIES {
        let dx = rng.below(span) as i64 - hop;
        let dz = rng.below(span) as i64 - hop;
        let z = home.z as i64 + dz;
        if (dx, dz) == (0, 0) || z < 0 || z >= c.depth as i64 {
            continue;
        }
        let Some(site) = highest_support(view, home.x as i64 + dx, z as u32) else {
            continue;
        };
        if site == home {
            continue;
        }
        last = Some(site);
        if ground_passes(view, ground, site, sc) {
            return Landing::Bank(site);
        }
    }
    last.map_or(Landing::Retry, Landing::Lost)
}

/// A standing-water margin for this species: settled standing water on the face or beside
/// it ([`standing_water_beside`]), and the face itself no deeper under it than the species
/// can stand in — the bank and the shallows, not the middle of the lake.
pub(crate) fn is_margin(view: &VoxelView<'_>, site: Site, sc: &SpeciesConfig) -> bool {
    standing_water_beside(view, site) > 0.0
        && view.standing_depth_m(site.x as i64, site.y, site.z) <= sc.drown_depth_m
}

/// **Water**: every column within `hop` — a water species' `hop` is its wide radius — whose
/// highest support face is a margin ([`is_margin`]) is a candidate, and the seed goes
/// **downhill first**: one drawn uniformly among the margins at or below the donor's own
/// face, and among the higher ones only if there are none. With no margin in reach the
/// seed is litter on the donor's own face.
fn water_target(
    view: &VoxelView<'_>,
    sc: &SpeciesConfig,
    home: Site,
    world_seed: u64,
    home_index: u64,
    tick: u64,
) -> Landing {
    let c = view.config;
    let hop = sc.hop as i64;
    let (mut down, mut up): (Vec<Site>, Vec<Site>) = (Vec::new(), Vec::new());
    for dz in -hop..=hop {
        let z = home.z as i64 + dz;
        if z < 0 || z >= c.depth as i64 {
            continue;
        }
        for dx in -hop..=hop {
            let Some(site) = highest_support(view, home.x as i64 + dx, z as u32) else {
                continue;
            };
            if site == home || !is_margin(view, site, sc) {
                continue;
            }
            let side = if site.y <= home.y { &mut down } else { &mut up };
            if !side.contains(&site) {
                side.push(site);
            }
        }
    }
    let pool = if down.is_empty() { &up } else { &down };
    if pool.is_empty() {
        return Landing::Lost(home);
    }
    let mut rng = Rng::keyed(DOMAIN_WATER, world_seed, home_index, tick);
    Landing::Bank(pool[rng.below(pool.len())])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::{DOMAIN_WIND, Rng, establishment_gates_on_substrate};
    use crate::{Dispersal, Flora, FloraConfig, Ground, SeedCohort, Site, Species, SpeciesConfig};
    use cubarium_voxel::DT;
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

    const HOUR_TICKS: u64 = 3600 * cubarium_voxel::TICK_HZ as u64;

    /// `width × depth` columns of 1 m voxels: bedrock at `y = 0`, soil at `y = 1..=2`
    /// holding `pore` of its capacity, air above, no rain. Every column's support face is
    /// `y = 2`, in open sky.
    fn slab(width: u32, depth: u32, pore: f64) -> World {
        let config = VoxelConfig {
            width,
            height: 8,
            depth,
            voxel_m: 1.0,
            seed: 5,
            rain_m_per_s: 0.0,
            ..VoxelConfig::default()
        };
        let mut w = World::empty(config);
        for z in 0..depth {
            for x in 0..width as i64 {
                for y in 1..=2u32 {
                    soil(&mut w, x, y, z, pore);
                }
            }
        }
        w
    }

    /// One air voxel turned to soil holding `pore` of its capacity: water in first, then
    /// the conversion, so nothing is displaced.
    fn soil(w: &mut World, x: i64, y: u32, z: u32, pore: f64) {
        let want = pore * Material::Soil.pore_capacity() * w.config().voxel_volume();
        if want > 0.0 {
            w.apply(WorldCommand::AddWater {
                x,
                y,
                z,
                volume_m3: want,
            });
        }
        w.apply(WorldCommand::SetMaterial {
            x,
            y,
            z,
            material: Material::Soil,
        });
    }

    /// Raise one column's two soil voxels to `pore`: the gate opening. Each voxel is
    /// turned to air (its pore water becomes free water in the cell), topped up, and
    /// turned back.
    fn wet(w: &mut World, x: i64, z: u32, pore: f64) {
        let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
        for y in 1..=2u32 {
            let add = (pore - w.view().pore_at(x, y, z)) * cap;
            assert!(add >= 0.0, "wet only raises the water");
            w.apply(WorldCommand::SetMaterial {
                x,
                y,
                z,
                material: Material::Air,
            });
            w.apply(WorldCommand::AddWater {
                x,
                y,
                z,
                volume_m3: add,
            });
            w.apply(WorldCommand::SetMaterial {
                x,
                y,
                z,
                material: Material::Soil,
            });
        }
        assert!((w.view().pore_at(x, 2, z) - pore).abs() < 1e-9);
    }

    fn residuals(flora: &Flora) -> (f64, f64, f64) {
        let v = flora.view();
        (
            v.organic() - v.ledger.expected_organic(),
            v.mineral() - v.ledger.expected_mineral(),
            v.energy() - v.ledger.expected_energy(),
        )
    }

    fn assert_conserved(flora: &Flora, when: &str) {
        let v = flora.view();
        let (o, n, e) = residuals(flora);
        assert!(
            o.abs() <= 1e-9 * v.organic().max(1.0),
            "{when}: organic {o}"
        );
        assert!(
            n.abs() <= 1e-9 * v.mineral().max(1.0),
            "{when}: mineral {n}"
        );
        assert!(e.abs() <= 1e-9 * v.energy().max(1.0), "{when}: energy {e}");
    }

    /// `n` whole seeds of `species` banked on `site` in one bin opening now, booked as
    /// seeded material the way a founder is, and put on the check wheel.
    fn inject(flora: &mut Flora, site: Site, species: Species, n: u64) {
        let sc = flora.config.species(species).clone();
        let organic = n as f64 * package_of(&sc);
        let mineral = sc.n_tissue * organic;
        let gi = match flora.ground.binary_search_by_key(&site, |g| g.site) {
            Ok(i) => i,
            Err(i) => {
                flora.ground.insert(i, Ground::new(site, 1.0));
                flora.ledger.seeded_mineral_in += 1.0;
                i
            }
        };
        let g = &mut flora.ground[gi];
        g.seeds.push(SeedCohort {
            species,
            organic,
            mineral,
            bin_start_tick: flora.tick,
        });
        g.seeds.sort_by_key(|c| (c.species, c.bin_start_tick));
        flora.ledger.seeded_organic_in += organic;
        flora.ledger.seeded_mineral_in += mineral;
        flora.ledger.seeded_energy_in += sc.energy_density * organic;
        schedule(flora, site);
    }

    /// The default check period, which every fixture here runs on.
    const PERIOD: u64 = 600;

    /// The first tick after `after` on which `site` is checked.
    fn next_check(site: Site, after: u64) -> u64 {
        let phase = check_phase(site, PERIOD);
        let t = after - after % PERIOD + phase;
        if t > after { t } else { t + PERIOD }
    }

    /// Step the plant layer (not the world) until its clock reads `tick`.
    fn run_to(flora: &mut Flora, world: &mut World, tick: u64) {
        assert!(flora.tick <= tick);
        while flora.tick < tick {
            flora.step(world);
        }
    }

    fn whole_seeds(flora: &Flora, site: Site, species: Species) -> f64 {
        let package = package_of(flora.config.species(species));
        flora
            .view()
            .ground_at(site)
            .map_or(0.0, |g| g.seed_organic(species) / package)
    }

    /// Test 1. A lone seed on a site whose gates fail **waits**: it is refused at its
    /// check, still there hours later, and germinates at the first check after the gate
    /// opens — not before it, and not on the tick the soil was wetted.
    #[test]
    fn a_lone_seed_waits_hours_and_germinates_at_the_first_check_after_the_gate_opens() {
        let mut config = FloraConfig::default();
        // This test is about waiting, not about attrition.
        config.bloomcrown.seed_attrition_per_s = 0.0;
        assert!(
            config.bloomcrown.seed_max_age_s >= 3.0 * 3600.0,
            "bloomcrown's bank must outlive the wait"
        );
        let mut world = slab(3, 1, 0.05); // under the wilting point: no available water, the gate is shut
        let mut flora = Flora::new(config);
        let site = Site { x: 1, y: 2, z: 0 };
        inject(&mut flora, site, Species::Bloomcrown, 1);

        let first = next_check(site, flora.tick);
        run_to(&mut flora, &mut world, first);
        assert_eq!(flora.view().ledger.establishments, 0, "the gate is shut");
        assert_eq!(whole_seeds(&flora, site, Species::Bloomcrown).round(), 1.0);

        // Two hours pass (the fixture moves the clock without running the model), and the
        // ground is wetted between checks.
        flora.tick += 2 * HOUR_TICKS;
        wet(&mut world, 1, 0, 0.6);
        let open = next_check(site, flora.tick);
        run_to(&mut flora, &mut world, open - 1);
        assert_eq!(
            flora.view().ledger.establishments,
            0,
            "a site is not tested between its checks"
        );
        flora.step(&mut world);
        assert_eq!(flora.tick, open);
        assert_eq!(
            flora.view().ledger.establishments,
            1,
            "it germinates at the check"
        );
        let born = flora.view().stand_at(site).expect("a stand");
        assert_eq!(born.species, Species::Bloomcrown);
        assert_eq!(born.wood, flora.config.bloomcrown.alive_min);
        assert_eq!(whole_seeds(&flora, site, Species::Bloomcrown), 0.0);
        assert_eq!(
            flora.view().ledger.seeds_germinated[Species::Bloomcrown.index()],
            1
        );
        assert_conserved(&flora, "after the germination");
    }

    /// Test 2. Attrition kills **whole seeds** — a cohort always holds a whole number of
    /// packages — and every seed it kills is in the litter with its mineral.
    #[test]
    fn attrition_removes_whole_seeds_and_conserves_organic_into_litter() {
        let mut config = FloraConfig::default();
        // The test's own rates: about four tenths of the seeds die per 30 s check, and
        // litter does not decompose, so the litter is exactly what the seeds left.
        config.bloomcrown.seed_attrition_per_s = 1.0 / 60.0;
        config.decomposition = 0.0;
        let package = package_of(&config.bloomcrown);
        let mut world = slab(3, 1, 0.05); // shut, so nothing germinates
        let mut flora = Flora::new(config);
        let site = Site { x: 1, y: 2, z: 0 };
        inject(&mut flora, site, Species::Bloomcrown, 40);

        let mut before = 40.0;
        let mut died_total = 0.0;
        let mut some_died_some_lived = false;
        for _ in 0..3 {
            let litter0 = flora.view().ground_at(site).unwrap().litter;
            let lmin0 = flora.view().ground_at(site).unwrap().litter_mineral;
            let t = next_check(site, flora.tick);
            run_to(&mut flora, &mut world, t);
            let now = whole_seeds(&flora, site, Species::Bloomcrown);
            assert!(
                (now - now.round()).abs() < 1e-9,
                "a fraction of a seed: {now}"
            );
            let died = before - now.round();
            assert!(died >= 0.0);
            some_died_some_lived |= died > 0.0 && now.round() > 0.0;
            let g = flora.view().ground_at(site).unwrap();
            assert!(
                (g.litter - litter0 - died * package).abs() < 1e-12,
                "{died} dead seeds are {} of litter, not {}",
                died * package,
                g.litter - litter0
            );
            let n_tissue = flora.config.bloomcrown.n_tissue;
            assert!((g.litter_mineral - lmin0 - died * package * n_tissue).abs() < 1e-12);
            died_total += died;
            before = now.round();
            assert_conserved(&flora, "after a check");
        }
        assert!(some_died_some_lived, "the draws were all or nothing");
        assert_eq!(
            flora.view().ledger.seeds_died[Species::Bloomcrown.index()] as f64,
            died_total
        );
    }

    /// Test 3. The wind kernel: fat-tailed and isotropic. Most seeds within three hops,
    /// a few percent past ten, never the donor's own column, and capped at the world.
    #[test]
    fn the_wind_kernel_is_mostly_near_with_a_few_percent_past_ten_hops() {
        let hop = 2u32;
        let n = 20_000u64;
        let (mut near, mut far) = (0u64, 0u64);
        // Signs of the nonzero offsets, per axis: [negative, positive].
        let mut signs = [[0u64; 2]; 2];
        for i in 0..n {
            let mut rng = Rng::keyed(DOMAIN_WIND, 1, i, 0);
            let (dx, dz) = wind_offset(&mut rng, hop, 1e9);
            assert!(
                (dx, dz) != (0, 0),
                "a wind seed never lands on its own column"
            );
            let d = ((dx * dx + dz * dz) as f64).sqrt();
            if d <= 3.0 * f64::from(hop) {
                near += 1;
            }
            if d > 10.0 * f64::from(hop) {
                far += 1;
            }
            for (axis, d) in [dx, dz].into_iter().enumerate() {
                if d != 0 {
                    signs[axis][usize::from(d > 0)] += 1;
                }
            }
        }
        let near = near as f64 / n as f64;
        let far = far as f64 / n as f64;
        assert!(near > 0.6, "only {near} within three hops");
        assert!((0.02..=0.1).contains(&far), "{far} past ten hops");
        for [neg, pos] in signs {
            let share = pos as f64 / (neg + pos) as f64;
            assert!((0.47..=0.53).contains(&share), "not isotropic: {signs:?}");
        }
        // Capped at the world.
        for i in 0..2_000u64 {
            let mut rng = Rng::keyed(DOMAIN_WIND, 2, i, 0);
            let (dx, dz) = wind_offset(&mut rng, hop, 5.0);
            assert!(((dx * dx + dz * dz) as f64).sqrt() <= 6.0, "({dx}, {dz})");
        }
    }

    /// Test 4. Spores land only on sites whose ground passes the species' gate — every
    /// gate but light — and the rain that finds none falls as litter.
    #[test]
    fn spores_land_only_on_sites_that_pass_the_gate() {
        // A checkerboard of 4 × 4 damp blocks: a velvetpad's 3 × 3 root box reads damp
        // enough (0.3) inside a block and too dry across the corners and outside.
        let (width, depth) = (16u32, 16u32);
        let mut world = slab(width, depth, 0.05);
        for z in 0..depth {
            for x in 0..width as i64 {
                if (x as u32 / 4 + z / 4) % 2 == 0 {
                    wet(&mut world, x, z, 0.6);
                }
            }
        }
        let mut config = FloraConfig::default();
        config.velvetpad.dispersal = Dispersal::Spores;
        config.velvetpad.hop = 5;
        let sc = config.velvetpad.clone();
        let view = world.view();
        let home = Site { x: 8, y: 2, z: 8 };
        let ground: Vec<Ground> = Vec::new();
        let (mut banked, mut lost) = (0, 0);
        for tick in 0..400u64 {
            match landing(&view, &ground, &sc, home, 11, tick) {
                Landing::Bank(site) => {
                    banked += 1;
                    assert_ne!(site, home);
                    let g = establishment_gates_on_substrate(&view, site, &sc, 0.0, 0.0);
                    assert!(
                        g.pore_ok && g.aeration_ok && g.depth_ok && g.standing_ok,
                        "a spore landed on {site:?}, which fails {g:?}"
                    );
                }
                Landing::Lost(_) => lost += 1,
                Landing::Retry => {}
            }
        }
        assert!(banked > 100, "only {banked} landed");
        assert!(lost > 0, "a sparse habitat should lose some of the rain");
    }

    /// Test 5. Water landings fall only on standing-water margins: settled water on the
    /// face or beside it, and the face no deeper than the species can stand in.
    #[test]
    fn water_landings_fall_only_at_standing_water_margins() {
        let (width, depth) = (14u32, 7u32);
        let config_w = VoxelConfig {
            width,
            height: 8,
            depth,
            voxel_m: 1.0,
            seed: 5,
            rain_m_per_s: 0.0,
            ..VoxelConfig::default()
        };
        let mut world = World::empty(config_w);
        // A pool: its floor one voxel down, x 6..=8, z 2..=4, a metre of water on it.
        let pool = |x: i64, z: u32| (6..=8).contains(&x) && (2..=4).contains(&z);
        for z in 0..depth {
            for x in 0..width as i64 {
                soil(&mut world, x, 1, z, 0.9);
                if !pool(x, z) {
                    soil(&mut world, x, 2, z, 0.5);
                }
            }
        }
        for z in 2..=4u32 {
            for x in 6..=8i64 {
                world.apply(WorldCommand::AddWater {
                    x,
                    y: 2,
                    z,
                    volume_m3: 1.0,
                });
            }
        }
        let mut config = FloraConfig::default();
        config.siphonreed.dispersal = Dispersal::Water;
        config.siphonreed.hop = 6;
        let sc = config.siphonreed.clone();
        let view = world.view();
        let margin = |s: Site| {
            crate::step::standing_water_beside(&view, s) > 0.0
                && view.standing_depth_m(i64::from(s.x), s.y, s.z) <= sc.drown_depth_m
        };
        // Some margins exist, and the pool floor (a metre deep) is not one of them.
        assert!(margin(Site { x: 5, y: 2, z: 3 }));
        assert!(!margin(Site { x: 7, y: 1, z: 3 }));
        assert!(!margin(Site { x: 1, y: 2, z: 3 }));
        let home = Site { x: 2, y: 2, z: 3 };
        let ground: Vec<Ground> = Vec::new();
        let mut banked = 0;
        for tick in 0..200u64 {
            match landing(&view, &ground, &sc, home, 11, tick) {
                Landing::Bank(site) => {
                    banked += 1;
                    assert!(
                        margin(site),
                        "a water seed landed off the margin at {site:?}"
                    );
                }
                other => panic!("margins are in reach, yet {other:?}"),
            }
        }
        assert_eq!(banked, 200);
    }

    /// Test 6. A runner daughter: on a free neighbouring cell, paid for out of the
    /// parent's reserve, nothing banked, and conserved.
    #[test]
    fn a_runner_daughter_is_adjacent_paid_from_the_reserve_and_conserved() {
        let mut config = FloraConfig::default();
        // The test's own numbers: every package is a runner, and one tick funds one.
        config.springturf.clonal_share = 1.0;
        config.springturf.propagule_rate = 1.0;
        config.springturf.donor_reserve_floor = 0.0;
        let sc = config.springturf.clone();
        let package = package_of(&sc);
        let mut world = slab(5, 1, 0.6);
        let mut flora = Flora::new(config);
        assert!(flora.apply(
            &world,
            crate::Command::Seed {
                x: 2,
                z: 0,
                species: Species::Springturf,
                wood: sc.wood_max,
            }
        ));
        let parent = Site { x: 2, y: 2, z: 0 };
        flora.step(&mut world);

        let v = flora.view();
        assert_eq!(v.stands.len(), 2, "one daughter");
        let daughter = v
            .stands
            .iter()
            .find(|s| s.site != parent)
            .expect("a daughter");
        let dx = (i64::from(daughter.site.x) - 2).abs();
        assert_eq!(dx, 1, "adjacent: {:?}", daughter.site);
        assert_eq!(daughter.species, Species::Springturf);
        assert_eq!(daughter.wood, sc.alive_min);
        assert!((daughter.material() - package).abs() < 1e-15);
        let slot = Species::Springturf.index();
        assert_eq!(v.ledger.clonal_births[slot], 1);
        assert_eq!(v.ledger.establishments, 1);
        // Paid out of the reserve: what the parent funded is the package plus what it is
        // still saving.
        let mother = v.stand_at(parent).expect("the parent");
        assert!(
            (v.ledger.propagule_funded[slot] - package - mother.parcel).abs() < 1e-15,
            "funded {} for a {package} package and {} saved",
            v.ledger.propagule_funded[slot],
            mother.parcel
        );
        assert!(
            v.ground.iter().all(|g| g.seeds.is_empty()),
            "a runner banks nothing"
        );
        assert_conserved(&flora, "after the runner");
    }

    /// Test 7. A banked site is tested only at its own check — except on the tick a
    /// shower ends, when every bank is.
    #[test]
    fn a_site_is_not_tested_between_checks_except_on_a_shower_flush() {
        let mut config = FloraConfig::default();
        config.bloomcrown.seed_attrition_per_s = 0.0;
        let mut world = slab(4, 1, 0.6); // open: a bloomcrown passes everywhere
        let mut flora = Flora::new(config);

        // Banked on the tick of its own check, so the whole period is between checks.
        let a = Site { x: 1, y: 2, z: 0 };
        let first = next_check(a, 0);
        run_to(&mut flora, &mut world, first);
        inject(&mut flora, a, Species::Bloomcrown, 1);
        run_to(&mut flora, &mut world, first + PERIOD - 1);
        assert_eq!(
            flora.view().ledger.establishments,
            0,
            "tested between checks"
        );
        flora.step(&mut world);
        assert_eq!(
            flora.view().ledger.establishments,
            1,
            "not tested at its check"
        );

        // A second bank, between its checks, and a shower that ends this tick.
        let b = Site { x: 3, y: 2, z: 0 };
        inject(&mut flora, b, Species::Bloomcrown, 1);
        let due = next_check(b, flora.tick);
        assert!(
            due > flora.tick + 1,
            "the fixture needs a tick that is not b's check"
        );
        flora.was_raining = true; // the world is dry: the shower has just ended
        flora.step(&mut world);
        assert!(flora.tick < due);
        assert_eq!(
            flora.view().ledger.establishments,
            2,
            "the flush did not test it"
        );
        assert!(flora.view().stand_at(b).is_some());
        assert_conserved(&flora, "after the flush");
    }

    /// The package a donor saves for is the material a germination needs to build a stand
    /// at exactly `alive_min` of wood, so `w_frac · package` must not land **under**
    /// `alive_min` in f64 — a newborn a hair under it would be born and die on its first
    /// tick. It is exact at every preset's placeholders, and this is the test a new preset
    /// has to keep passing: round 4 added three, whose packages are 0.015, 0.025 and 0.0375
    /// against the original pair's 0.05.
    #[test]
    fn one_package_builds_a_stand_at_exactly_alive_min() {
        let config = FloraConfig::default();
        for species in Species::ALL {
            let sc = config.species(species).clone();
            let package = package_of(&sc);
            let want = sc.alive_min / 0.4;
            assert!(
                (package - want).abs() < 1e-15,
                "{}: a {package} package for a {want} split",
                species.name()
            );
            let wood = sc.propagule_split[0] * package;
            assert!(
                wood >= sc.alive_min,
                "a package builds {wood} of wood, under {}",
                sc.alive_min
            );
            assert!(
                wood - sc.alive_min <= 1e-15,
                "a package builds {wood}, which is not alive_min {}",
                sc.alive_min
            );
        }
        // A species that cannot put anything into wood has no package at all, and
        // `propagate` and germination both skip it rather than dividing by zero.
        let mut odd = SpeciesConfig::bloomcrown();
        odd.propagule_split = [0.0, 0.5, 0.5];
        assert_eq!(package_of(&odd), 0.0);
    }

    /// The gap lottery: weighted by whole packages, reproducible per (world, site, tick),
    /// and **blind to the order the candidates arrive in** — which is R4.5's requirement,
    /// since that order is `Species::ALL` today and could be a storage order tomorrow.
    ///
    /// Two hundred seeded draws at weights 1 and 3: both species win somewhere, the counts
    /// sit near the weights, and swapping the two candidates round gives the identical
    /// winner on all two hundred.
    #[test]
    fn the_gap_lottery_follows_the_weights_and_not_the_order_it_is_handed() {
        let (b, u) = (Species::Bloomcrown, Species::Umbrellafrond);
        let mut wins = [0usize; 2];
        let mut swapped_disagreements = 0;
        for site in 0..200u64 {
            let one = lottery(11, site, 3, &[(b, 1), (u, 3)]).expect("two candidates");
            let other = lottery(11, site, 3, &[(u, 3), (b, 1)]).expect("two candidates");
            if one != other {
                swapped_disagreements += 1;
            }
            wins[one.index()] += 1;
        }
        assert_eq!(
            swapped_disagreements, 0,
            "the order the candidates came in moved the winner"
        );
        assert!(
            wins[0] > 0 && wins[1] > 0,
            "one species never won: {wins:?}"
        );
        // 1:3 over 200 draws is 50 against 150; anything inside 35..65 is the weights and
        // not the enum order, which would be 200 against 0.
        assert!(
            (35..=65).contains(&wins[0]),
            "weights 1 and 3 gave {wins:?}"
        );
        assert_eq!(wins[0] + wins[1], 200);

        // Degenerate cases: nothing to draw among, and one candidate that always wins
        // whatever its weight.
        assert_eq!(lottery(11, 0, 0, &[]), None);
        assert_eq!(lottery(11, 0, 0, &[(u, 1)]), Some(u));
        assert_eq!(
            lottery(11, 0, 0, &[(b, 0), (u, 0)]),
            None,
            "no packages, no winner"
        );
    }

    /// Spending a seed takes it from the **oldest** bin that holds one: a one-seed bin
    /// goes whole with its mineral, a larger one gives up  of itself and keeps its
    /// start, and another species' bins are not touched.
    #[test]
    fn spending_a_seed_takes_it_from_the_oldest_bin_that_holds_one() {
        let package = package_of(&SpeciesConfig::bloomcrown());
        let mut g = Ground::new(Site { x: 0, y: 1, z: 0 }, 0.0);
        for (start, n) in [(0u64, 1.0), (10, 2.0), (20, 3.0)] {
            g.seeds.push(SeedCohort {
                species: Species::Bloomcrown,
                organic: n * package,
                mineral: 0.02 * n * package,
                bin_start_tick: start,
            });
        }
        g.seeds.push(SeedCohort {
            species: Species::Umbrellafrond,
            organic: 1.0,
            mineral: 0.02,
            bin_start_tick: 0,
        });
        let (organic, mineral) = spend_seed(&mut g, Species::Bloomcrown, package).expect("a seed");
        assert_eq!((organic, mineral), (package, 0.02 * package));
        assert_eq!(g.seeds[0].bin_start_tick, 10, "the one-seed bin went whole");
        let (organic, _) = spend_seed(&mut g, Species::Bloomcrown, package).expect("a seed");
        assert!((organic - package).abs() < 1e-15);
        assert_eq!(
            g.seeds[0].bin_start_tick, 10,
            "a part-spent bin keeps its start"
        );
        assert!((g.seeds[0].organic - package).abs() < 1e-15);
        for _ in 0..4 {
            assert!(spend_seed(&mut g, Species::Bloomcrown, package).is_some());
        }
        assert_eq!(
            spend_seed(&mut g, Species::Bloomcrown, package),
            None,
            "six seeds, no more"
        );
        assert_eq!(g.seeds.len(), 1);
        assert_eq!((g.seeds[0].organic, g.seeds[0].mineral), (1.0, 0.02));
    }

    /// The same allocation against **every one of the five presets' own splits**, on the value Astra's three
    /// bins produce and on an exact package: the wood is `alive_min` on the nose, no stock
    /// is negative, the total is preserved, and the foliage and reserve are the intended
    /// fractions to within the rounding that is being corrected. The last two cases are the
    /// degenerate splits a future preset could bring — no reserve at all, and a bank that
    /// somehow holds less than `alive_min`.
    #[test]
    fn a_newborn_s_wood_is_exactly_alive_min_for_every_split() {
        let config = FloraConfig::default();
        for species in Species::ALL {
            let sc = config.species(species).clone();
            let [w_frac, p_frac, q_frac] = sc.propagule_split;
            assert!(
                (w_frac + p_frac + q_frac - 1.0).abs() < 1e-15,
                "the split sums to one"
            );
            assert!(sc.alive_min <= sc.wood_max, "alive_min over wood_max");
            for organic in [0.04999999999999999, package_of(&sc), 0.2] {
                let (wood, foliage, reserve) = newborn_stocks(&sc, organic);
                assert_eq!(wood, sc.alive_min, "wood {wood} for {organic} of material");
                assert!(
                    foliage >= 0.0 && reserve >= 0.0,
                    "{wood} {foliage} {reserve}"
                );
                assert!(
                    ((wood + foliage + reserve) - organic).abs() <= 4e-18,
                    "{wood} + {foliage} + {reserve} against {organic} paid"
                );
                assert!(
                    (foliage - p_frac * organic).abs() <= 1e-17,
                    "foliage {foliage}"
                );
                if organic <= package_of(&sc) {
                    assert!(
                        (reserve - q_frac * organic).abs() <= 1e-17,
                        "reserve {reserve}"
                    );
                }
            }
        }
        // A split with nothing in the reserve: the remainder is zero and never negative.
        let mut dry = SpeciesConfig::bloomcrown();
        dry.propagule_split = [0.5, 0.5, 0.0];
        let (wood, foliage, reserve) = newborn_stocks(&dry, package_of(&dry));
        assert_eq!(wood, dry.alive_min);
        assert!(reserve >= 0.0 && reserve < 1e-17, "reserve {reserve}");
        assert!((wood + foliage + reserve - package_of(&dry)).abs() <= 4e-18);
        // And a bank under `alive_min`, which the candidate filter cannot produce: the wood
        // takes all of it and the stand is born dying rather than born rich.
        let (wood, foliage, reserve) = newborn_stocks(&dry, 0.01);
        assert_eq!((wood, foliage, reserve), (0.01, 0.0, 0.0));
    }

    /// The bank's size bound is by construction and not by a cap: a lifetime in bins of
    /// `ceil(lifetime / seed_cohorts_max)` holds at most `seed_cohorts_max + 1` of them,
    /// whatever the two numbers are and whether or not they divide.
    #[test]
    fn the_bin_width_bounds_a_bank_at_the_cohort_cap_plus_one() {
        for cap in 1..=12usize {
            for &life_s in &[0.05, 0.1, 1.0, 2.0, 7.0, 600.0] {
                let mut sc = SpeciesConfig::bloomcrown();
                sc.seed_max_age_s = life_s;
                sc.seed_cohorts_max = cap;
                let w = bin_ticks(&sc);
                assert!(w >= 1, "a zero-width bin at cap {cap}, life {life_s}");
                // Every bin start a landing can produce, over one lifetime of ticks: the
                // bins alive at once are the distinct starts inside the lifetime, plus the
                // one the oldest is expiring out of.
                let life_ticks = (life_s / DT) as u64;
                let live = life_ticks / w + 1;
                assert!(
                    live <= cap as u64 + 1,
                    "cap {cap}, life {life_s}: {live} live bins of {w} ticks"
                );
            }
        }
        // Bloomcrown's own: 8 h over 4 bins is 2 h, 144,000 ticks, exactly.
        let bloom = SpeciesConfig::bloomcrown();
        assert_eq!(bin_ticks(&bloom), 144_000);
        assert_eq!(bin_start(0, &bloom), 0);
        assert_eq!(bin_start(143_999, &bloom), 0);
        assert_eq!(bin_start(144_000, &bloom), 144_000);
    }

    // ------------------------------------------------ package SU: succession

    /// Plant a founder of `species` on column `x` of a one-row slab.
    fn founder(flora: &mut Flora, world: &World, x: i64, species: Species, wood: f64) {
        assert!(flora.apply(
            world,
            crate::Command::Seed {
                x,
                z: 0,
                species,
                wood
            }
        ));
    }

    /// SU 1. A woody seed banked under a **ground cover** comes up through it at a check
    /// whose draw says so: the cover dies an "overtopped" death, booked like any other —
    /// its wood to dead wood, its foliage and reserve to litter — and the woody seedling
    /// stands on the site. One stand per site, and every unit accounted for.
    #[test]
    fn a_woody_seed_comes_up_through_ground_cover_and_the_cover_is_overtopped() {
        let mut config = FloraConfig::default();
        // The test's own: a certain draw, so the check names the tick; no attrition, and a
        // cover that sends nothing, so the site holds exactly what the test put there.
        config.bloomcrown.overtop_per_check = 1.0;
        config.bloomcrown.seed_attrition_per_s = 0.0;
        config.springturf.propagule_rate = 0.0;
        assert!(config.springturf.ground_cover, "springturf is a ground cover");
        let mut world = slab(4, 1, 0.6);
        let mut flora = Flora::new(config);
        let site = Site { x: 1, y: 2, z: 0 };
        founder(&mut flora, &world, 1, Species::Springturf, 0.05);
        inject(&mut flora, site, Species::Bloomcrown, 1);
        let t = next_check(site, flora.tick);
        run_to(&mut flora, &mut world, t - 1);
        assert_eq!(flora.view().stand_at(site).unwrap().species, Species::Springturf);
        flora.step(&mut world);
        let v = flora.view();
        let now = v.stand_at(site).expect("a stand");
        assert_eq!(now.species, Species::Bloomcrown, "the woody seedling came up through");
        assert_eq!(v.stands.iter().filter(|s| s.site == site).count(), 1);
        assert_eq!(v.ledger.overtopped[Species::Springturf.index()], 1);
        assert_eq!(v.ledger.deaths, 1, "the cover's death is booked");
        assert_eq!(v.ledger.seeds_germinated[Species::Bloomcrown.index()], 1);
        let g = v.ground_at(site).unwrap();
        assert!(g.dead_wood > 0.0 && g.litter > 0.0, "the cover's tissue: {g:?}");
        assert_conserved(&flora, "after the overtopping");
    }

    /// SU 1, the other half: only a **woody** seed overtops, and only a **ground cover**
    /// is overtopped. A ground cover's seed waits under another cover, and a woody seed
    /// waits under a stand that is not a cover.
    #[test]
    fn only_woody_seeds_overtop_and_only_ground_covers_are_overtopped() {
        let mut config = FloraConfig::default();
        config.bloomcrown.overtop_per_check = 1.0;
        for sc in [&mut config.bloomcrown, &mut config.springturf] {
            sc.seed_attrition_per_s = 0.0;
        }
        config.velvetpad.propagule_rate = 0.0;
        config.umbrellafrond.propagule_rate = 0.0;
        assert!(!config.springturf.overtop_per_check.is_normal(), "springturf is no tree");
        assert!(!config.umbrellafrond.ground_cover, "a frond is not a ground cover");
        let mut world = slab(6, 1, 0.6);
        let mut flora = Flora::new(config);
        founder(&mut flora, &world, 1, Species::Velvetpad, 0.15);
        founder(&mut flora, &world, 4, Species::Umbrellafrond, 0.3);
        let (pad, frond) = (Site { x: 1, y: 2, z: 0 }, Site { x: 4, y: 2, z: 0 });
        inject(&mut flora, pad, Species::Springturf, 3);
        inject(&mut flora, frond, Species::Bloomcrown, 3);
        let t = next_check(pad, flora.tick).max(next_check(frond, flora.tick));
        run_to(&mut flora, &mut world, t);
        let v = flora.view();
        assert_eq!(v.stand_at(pad).unwrap().species, Species::Velvetpad);
        assert_eq!(v.stand_at(frond).unwrap().species, Species::Umbrellafrond);
        assert_eq!(v.ledger.overtopped.iter().sum::<u64>(), 0);
        assert_eq!(v.ledger.establishments, 0);
    }

    /// SU 1: coming up through a mat is **harder** than taking a gap — a per-check draw
    /// below one, keyed like every other draw. The frequency is the chance.
    #[test]
    fn the_overtop_chance_is_a_keyed_per_check_draw() {
        let n = 20_000u64;
        let hits = (0..n).filter(|&t| overtops(3, 77, t, 0.02)).count() as f64 / n as f64;
        assert!((0.015..=0.025).contains(&hits), "{hits}");
        assert!((0..1000).all(|t| !overtops(3, 77, t, 0.0)));
        assert!((0..1000).all(|t| overtops(3, 77, t, 1.0)));
        assert_eq!(overtops(3, 77, 5, 0.5), overtops(3, 77, 5, 0.5));
        let config = FloraConfig::default();
        for species in [Species::Bloomcrown, Species::Vaulttree, Species::Lanternberry] {
            let p = config.species(species).overtop_per_check;
            assert!(p > 0.0 && p < 1.0, "{}: {p}", species.name());
        }
    }

    /// SU 2. Shrubs and trees carry **fewer, bigger seeds**: a seed is `seed_mass` minimum
    /// packages, the parent pays for all of it, and the seedling it starts is bigger —
    /// wood `w_frac` of the seed, not the bare `alive_min`. Ground covers keep one.
    #[test]
    fn a_bigger_seed_costs_its_mass_and_starts_a_bigger_seedling() {
        let config = FloraConfig::default();
        for species in [Species::Springturf, Species::Velvetpad, Species::Stonecushion] {
            assert_eq!(config.species(species).seed_mass, 1, "{}", species.name());
        }
        for species in [Species::Bloomcrown, Species::Vaulttree, Species::Lanternberry] {
            let sc = config.species(species);
            assert!(sc.seed_mass >= 2, "{}: {}", species.name(), sc.seed_mass);
            let minimum = sc.alive_min / sc.propagule_split[0];
            let seed = package_of(sc);
            assert!((seed - f64::from(sc.seed_mass) * minimum).abs() < 1e-15);
            let (wood, foliage, reserve) = newborn_stocks(sc, seed);
            assert!(wood > sc.alive_min, "{}: a bigger seedling", species.name());
            assert!((wood - sc.propagule_split[0] * seed).abs() < 1e-15);
            assert!(((wood + foliage + reserve) - seed).abs() < 1e-15);
        }
    }

    /// SU 2. The gap lottery weighs **seed count × seed mass**: one seed four times the
    /// minimum weighs what four minimum seeds do.
    #[test]
    fn the_gap_lottery_weighs_seed_count_by_seed_mass() {
        let mut big = SpeciesConfig::vaulttree();
        big.seed_mass = 4;
        let small = SpeciesConfig::springturf();
        assert_eq!(lottery_weight(1, &big), 4);
        assert_eq!(lottery_weight(4, &small), 4);
        assert_eq!(lottery_weight(3, &small), 3);
    }

    /// SU 3. The ground covers' shortcuts, reined in: a velvetpad's spore rain tries
    /// **two** faces, not eight — so on a patchy floor more of it falls where nothing
    /// grows — and a springturf sends 0.3 of its packages as runners, not 0.5.
    #[test]
    fn velvetpad_spores_try_twice_and_springturf_runs_less() {
        let config = FloraConfig::default();
        assert_eq!(config.velvetpad.spore_tries, 2);
        assert_eq!(config.umbrellafrond.spore_tries, 8);
        assert_eq!(config.springturf.clonal_share, 0.3);
        let (width, depth) = (16u32, 16u32);
        let mut world = slab(width, depth, 0.05);
        for z in 0..depth {
            for x in 0..width as i64 {
                if (x as u32 / 4 + z / 4) % 2 == 0 {
                    wet(&mut world, x, z, 0.6);
                }
            }
        }
        let view = world.view();
        let home = Site { x: 8, y: 2, z: 8 };
        let ground: Vec<Ground> = Vec::new();
        let banked = |tries: u32| {
            let mut sc = config.velvetpad.clone();
            sc.hop = 5;
            sc.spore_tries = tries;
            (0..400u64)
                .filter(|&t| matches!(landing(&view, &ground, &sc, home, 11, t), Landing::Bank(_)))
                .count()
        };
        let (two, eight) = (banked(2), banked(8));
        assert!(two < eight, "two tries banked {two}, eight {eight}");
        assert!(two > 0, "two tries still land some");
    }
}
