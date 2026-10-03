//! The full-roster search's groundwork (`design/handoffs/roster-search-groundwork-2026-10-02.md`;
//! proposal `design/ecology-timescales-2026-10-02.md` Part 3, Astra A5, A6, A10, A11): the
//! night-endurance and food-access screens, the per-lineage targets, and `roster.toml`'s
//! decode. Arithmetic only, no ticks.

use cubarium_voxel::weather::DayMode;
use cubarium_voxel_fauna::{Activity, FaunaConfig, Founder};
use cubarium_voxel_flora::{FloraConfig, Species, Trophic};

use super::ecology::EcologyParams;
use super::screen::{
    FoodSpec, NightSpec, ScreenSpec, basal_food_per_day, night_endurance_s, roster, sleep_bouts_s,
    stand_product_per_day,
};
use super::targets::{LineageOutcome, Rule, StageSpec, judge, replenishment};

const VOXEL_M: f64 = 0.125;

/// The terrarium's world: 256 x 48 columns of 0.125 m, the 120-min cycle, 0.67 daylight.
fn world() -> cubarium_voxel::Config {
    let mut c = cubarium_voxel::Config::default();
    c.width = 256;
    c.depth = 48;
    c.voxel_m = VOXEL_M;
    c.day.mode = DayMode::Cycle;
    c.day.length_min = 120.0;
    c.day.daylight = 0.67;
    c
}

/// Every screen on, nothing about R/S/F.
fn spec() -> ScreenSpec {
    ScreenSpec {
        night: Some(NightSpec::default()),
        food: Some(FoodSpec::default()),
        ..ScreenSpec::default()
    }
}

/// Only `only` founded (the rest at 0), each at its default count or `n`.
fn fauna_with(only: &[(Founder, usize)]) -> FaunaConfig {
    let mut f = FaunaConfig::default();
    for l in Founder::ALL {
        f.founders[l.index()].founding_count = 0;
    }
    for (l, n) in only {
        f.founders[l.index()].founding_count = *n;
    }
    f
}

fn flora() -> FloraConfig {
    FloraConfig::for_voxel_size(VOXEL_M)
}

fn refusals_naming(r: &super::screen::RosterScreen, what: &str, who: &str) -> Vec<String> {
    r.refusals()
        .into_iter()
        .filter(|s| s.starts_with(what) && s.contains(who))
        .collect()
}

// --- Night endurance ---------------------------------------------------------------

/// The proposal's worked numbers (1a, Astra A2): c 0.5, m 0.001, h 0.9, f0 0.8 give a
/// usable 0.35 body of reserve: 700 s at s 0.5 and 1,166.67 s at s 0.3. Exact.
#[test]
fn night_endurance_matches_the_worked_numbers() {
    // An empty stomach: the reserve alone.
    let a = night_endurance_s(0.5, 0.001, 0.5, 0.9, 0.8, 0.0);
    let b = night_endurance_s(0.5, 0.001, 0.3, 0.9, 0.8, 0.0);
    assert!((a - 700.0).abs() < 1e-9, "{a}");
    assert!((b - 3500.0 / 3.0).abs() < 1e-9, "{b}");
    // Woken at 0.5 hunger from 0.8: only 0.3 of the reserve is usable.
    assert!((night_endurance_s(0.5, 0.001, 0.5, 0.5, 0.8, 0.0) - 300.0).abs() < 1e-9);
    // Hunger already past waking at onset: no sleep at all, never negative.
    assert_eq!(night_endurance_s(0.5, 0.001, 0.5, 0.1, 0.8, 0.0), 0.0);
    // The stomach adds what it digests: 0.01 body more at a burn of 5e-4 /s is 20 s.
    assert!((night_endurance_s(0.5, 0.001, 0.5, 0.9, 0.8, 0.01) - 720.0).abs() < 1e-9);
    assert!((night_endurance_s(0.5, 0.001, 0.5, 0.1, 0.8, 0.01) - 20.0).abs() < 1e-9);
}

/// The sleeping phase is the clock's, by the activity's own lines (0.3; crepuscular
/// 0.05 / 0.95 in two bouts): on the 120-min, 0.67 cycle a diurnal body sleeps one bout of
/// about 2,304 s and a nocturnal one about 4,896 s (the proposal's "40 and 80 min" were
/// approximate). With the clock off nobody sleeps.
#[test]
fn sleeping_phases_follow_the_clock_and_the_activity_lines() {
    let w = world();
    let life = FaunaConfig::default().life;
    let d = sleep_bouts_s(&w.day, Activity::Diurnal, &life);
    let n = sleep_bouts_s(&w.day, Activity::Nocturnal, &life);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(n.len(), 1, "{n:?}");
    assert!((d[0] - 2304.0).abs() < 1.0, "diurnal sleeps {} s", d[0]);
    assert!((n[0] - 4896.0).abs() < 1.0, "nocturnal sleeps {} s", n[0]);
    assert!((d[0] + n[0] - 7200.0).abs() < 1e-6, "the two phases tile the day");
    let c = sleep_bouts_s(&w.day, Activity::Crepuscular, &life);
    assert_eq!(c.len(), 2, "crepuscular sleeps twice: {c:?}");
    assert!(sleep_bouts_s(&w.day, Activity::Always, &life).is_empty());
    let mut off = w.clone();
    off.day.mode = DayMode::Off;
    assert!(sleep_bouts_s(&off.day, Activity::Diurnal, &life).is_empty());
}

/// The pre-night frondgrazer (c 0.5, m 0.001, s 0.5, h 0.5, an empty stomach) outlasts
/// 300 s of its 2,304-s night: refused, by name. Upkeep /5 and wake hunger 0.9 give
/// 3,500 s >= 1.2 x 2,304: passes. (The gut's stomach in the guard:
/// `gut_screen_tests::the_night_guard_counts_the_digestible_stomach`.)
#[test]
fn the_night_screen_refuses_a_short_sleeper_and_passes_a_long_one() {
    let w = world();
    let mut fauna = fauna_with(&[(Founder::Browser, 8)]);
    // Hand-built: the pre-night frondgrazer (the code default is 0.9 since `night`), at
    // the pre-gut upkeep (0.001 /s), with nothing in its stomach.
    fauna.founders[Founder::Browser.index()].wake_hunger = 0.5;
    fauna.founders[Founder::Browser.index()].sleep_upkeep = 0.5;
    fauna.founders[Founder::Browser.index()].core.maintenance_per_s = 0.001;
    fauna.founders[Founder::Browser.index()].gut.s0 = 0.0;
    let r = roster(&flora(), &fauna, &w, &spec());
    let night = refusals_naming(&r, "night", "frondgrazer");
    assert_eq!(night.len(), 1, "{:?}", r.refusals());
    let row = r.night.iter().find(|n| n.lineage == "frondgrazer").expect("a row");
    assert!(!row.passes && (row.endurance_s - 300.0).abs() < 1e-9, "{row:?}");
    let b = &mut fauna.founders[Founder::Browser.index()];
    b.core.maintenance_per_s /= 5.0;
    b.wake_hunger = 0.9;
    let r = roster(&flora(), &fauna, &w, &spec());
    let row = r.night.iter().find(|n| n.lineage == "frondgrazer").expect("a row");
    assert!(row.passes, "{row:?}");
    assert!(refusals_naming(&r, "night", "frondgrazer").is_empty(), "{:?}", r.refusals());
}

// --- Food access ---------------------------------------------------------------------

/// Astra A5's seedporter, in the same units on both sides: basal food 4.280544 ou/day
/// (0.356 x 0.001 x (4824 + 0.5 x 2376) / 0.5) against a stand's 0.55368 ou/day (1e-4 x
/// (4824 + 0.3 x 2376)): 7.731 stands.
#[test]
fn the_fruit_arithmetic_matches_the_worked_numbers() {
    let need = basal_food_per_day(0.001, 0.356, 0.5, 4824.0, 2376.0, 0.5);
    let stand = stand_product_per_day(1e-4, 4824.0, 2376.0, 0.3);
    assert!((need - 4.280544).abs() < 1e-9, "{need}");
    assert!((stand - 0.55368).abs() < 1e-12, "{stand}");
    assert!((need / stand - 7.731).abs() < 1e-3, "{}", need / stand);
}

/// A lineage alone in a world with its food, and the same world with that food's source
/// switched off: passes, then refused, naming the lineage and the food.
fn food_case(lineage: Founder, food: &str, fauna: FaunaConfig, rich: FloraConfig, poor: FloraConfig) {
    let w = world();
    let mut s = spec();
    s.night = None;
    let ok = roster(&rich, &fauna, &w, &s);
    let row = ok.food.iter().find(|r| r.lineage == lineage.name()).expect("a food row");
    assert!(row.passes, "{} on {food} should pass: {row:?}", lineage.name());
    assert!(refusals_naming(&ok, "food", lineage.name()).is_empty(), "{:?}", ok.refusals());
    let bad = roster(&poor, &fauna, &w, &s);
    let named = refusals_naming(&bad, "food", lineage.name());
    assert_eq!(named.len(), 1, "{:?}", bad.refusals());
    assert!(named[0].contains(food), "{named:?}");
}

fn scaled(mut f: FloraConfig, edit: impl Fn(&mut cubarium_voxel_flora::SpeciesConfig)) -> FloraConfig {
    for s in Species::ALL {
        edit(f.species_mut(s));
    }
    f
}

#[test]
fn foliage_access_is_by_the_mouth_band() {
    let rich = scaled(flora(), |s| {
        s.foliage_rate *= 1000.0;
        s.assimilation *= 1000.0;
    });
    let poor = scaled(flora(), |s| {
        if s.trophic == Trophic::Photo {
            s.foliage_rate = 0.0;
        }
    });
    food_case(Founder::Browser, "foliage", fauna_with(&[(Founder::Browser, 8)]), rich.clone(), poor.clone());
    food_case(Founder::Loftstrider, "foliage", fauna_with(&[(Founder::Loftstrider, 2)]), rich, poor);
}

/// The loftstrider's band starts at 0.8 m: a world of turf alone (one cell tall) feeds a
/// frondgrazer and not a loftstrider.
#[test]
fn a_crown_below_the_band_is_not_food() {
    let only_turf = scaled(flora(), |s| {
        if s.trophic == Trophic::Photo && s.crown_height_m[1] > 0.5 {
            s.founder_per_m2 = 0.0;
        }
        s.foliage_rate *= 1000.0;
        s.assimilation *= 1000.0;
    });
    let w = world();
    let mut s = spec();
    s.night = None;
    let r = roster(&only_turf, &fauna_with(&[(Founder::Browser, 8), (Founder::Loftstrider, 2)]), &w, &s);
    let get = |n: &str| r.food.iter().find(|x| x.lineage == n).expect("row").clone();
    assert!(get("frondgrazer").passes, "{:?}", get("frondgrazer"));
    assert!(!get("loftstrider").passes, "{:?}", get("loftstrider"));
}

#[test]
fn litter_access() {
    let rich = scaled(flora(), |s| s.senescence *= 100.0);
    let poor = scaled(flora(), |s| s.senescence = 0.0);
    food_case(Founder::Blind, "litter", fauna_with(&[(Founder::Blind, 8)]), rich, poor);
}

#[test]
fn glowcap_cap_access() {
    let mut rich = scaled(flora(), |s| s.senescence = 0.0);
    {
        let g = rich.species_mut(Species::Glowcap);
        g.foliage_rate *= 100.0;
        g.founder_per_m2 = g.founder_per_m2.max(0.1);
        g.founder_cap = g.founder_cap.max(10);
    }
    let mut poor = rich.clone();
    poor.species_mut(Species::Glowcap).foliage_rate = 0.0;
    food_case(Founder::Capgnawer, "cap_tissue", fauna_with(&[(Founder::Capgnawer, 6)]), rich, poor);
}

#[test]
fn glassfilm_access() {
    let mut rich = flora();
    rich.film_growth_per_s *= 10.0;
    let mut poor = flora();
    poor.film_growth_per_s = 0.0;
    food_case(Founder::RippleSnail, "film", fauna_with(&[(Founder::RippleSnail, 8)]), rich, poor);
}

#[test]
fn nectar_access() {
    let rich = scaled(flora(), |s| s.nectar_per_s *= 100.0);
    let mut poor = scaled(flora(), |s| s.nectar_per_s = 0.0);
    poor.latticevine.nectar = 0.0;
    food_case(Founder::Bellwing, "nectar", fauna_with(&[(Founder::Bellwing, 6)]), rich, poor);
}

#[test]
fn fruit_access() {
    let rich = scaled(flora(), |s| s.fruit_per_s *= 100.0);
    let mut poor = scaled(flora(), |s| s.fruit_per_s = 0.0);
    poor.latticevine.fruit = 0.0;
    food_case(Founder::Seedporter, "fruit", fauna_with(&[(Founder::Seedporter, 4)]), rich, poor);
}

/// A carnivore's carrion is its prey's production: the chorister with frondgrazers
/// founded passes (on a cheap chorister), and with none is refused, on carrion.
#[test]
fn carrion_access_is_the_prey_list() {
    let w = world();
    let mut s = spec();
    s.night = None;
    let mut fed = fauna_with(&[(Founder::Chorister, 1), (Founder::Browser, 40)]);
    fed.founders[Founder::Chorister.index()].core.maintenance_per_s /= 20.0;
    let r = roster(&flora(), &fed, &w, &s);
    let row = r.food.iter().find(|x| x.lineage == "chorister").expect("row");
    assert!(row.passes, "{row:?}");
    let mut starved = fed;
    starved.founders[Founder::Browser.index()].founding_count = 0;
    let r = roster(&flora(), &starved, &w, &s);
    let named = refusals_naming(&r, "food", "chorister");
    assert_eq!(named.len(), 1, "{:?}", r.refusals());
    assert!(named[0].contains("carrion"), "{named:?}");
}

/// Predator balance is a readout (Astra A6): a world whose prey supports fewer predators
/// than it founds refuses nothing for it.
#[test]
fn predator_balance_is_a_readout_not_a_refusal() {
    let w = world();
    let s = ScreenSpec {
        night: None,
        food: Some(FoodSpec { min_ratio: 0.0, ..FoodSpec::default() }),
        ..ScreenSpec::default()
    };
    // At the gut's daily need two grazers' carrion supports ~23 choristers: eight packs
    // of four are more than that.
    let fauna = fauna_with(&[(Founder::Chorister, 8), (Founder::Browser, 2)]);
    let r = roster(&flora(), &fauna, &w, &s);
    let b = r.balance.iter().find(|b| b.predator == "chorister").expect("a balance row");
    assert!(b.supported < b.predators as f64, "{b:?}");
    assert!(r.refusals().is_empty(), "{:?}", r.refusals());
}

/// With `r_readout` (roster.toml, Fable 2026-10-02) R is reported and never refuses or
/// pays; S and F stay hard.
#[test]
fn r_is_a_readout_when_the_file_says_so() {
    let mut spec = ScreenSpec { r_readout: true, ..ScreenSpec::default() };
    let mut s = super::screen::rsf(&flora(), &FaunaConfig::default(), VOXEL_M, &spec);
    (s.r, s.s, s.f) = (5.0, 0.1, 20.0);
    assert!(s.passes(&spec), "{:?}", s.failures(&spec));
    assert_eq!(spec.verdict(&s), super::screen::Verdict::Inside);
    assert_eq!(s.distance(&spec), 0.0);
    (s.r, s.s, s.f) = (5.0, 5.0, 20.0);
    assert!(!s.passes(&spec) && s.failures(&spec).iter().all(|x| x.starts_with('S')));
    spec.r_readout = false;
    (s.r, s.s, s.f) = (5.0, 0.1, 20.0);
    assert!(!s.passes(&spec));
}

/// Screens off (cotrain.toml has neither table): no rows, no refusals.
#[test]
fn without_the_tables_the_roster_screens_are_off() {
    let r = roster(&flora(), &FaunaConfig::default(), &world(), &ScreenSpec::default());
    assert!(r.night.is_empty() && r.food.is_empty() && r.refusals().is_empty());
}

// --- Targets ------------------------------------------------------------------------

fn stage() -> StageSpec {
    StageSpec::default()
}

fn outcome(f: Founder) -> LineageOutcome {
    LineageOutcome::new(f, 8)
}

/// Readiness never substitutes for a required birth (Astra A10): a lineage held fully
/// ready with no birth fails, and pays at least half the term.
#[test]
fn readiness_without_a_birth_fails_a_birth_required_target() {
    let mut s = stage();
    s.birth_required[Founder::Browser.index()] = true;
    let mut o = outcome(Founder::Browser);
    o.alive_end = 8;
    o.readiness = 1.0;
    judge(&mut o, &s, true);
    assert_eq!(o.rule, Rule::Birth);
    assert!(!o.met, "{o:?}");
    assert!(o.shortfall >= 0.5, "{o:?}");
    let mut born = outcome(Founder::Browser);
    born.alive_end = 9;
    born.late_births = 1;
    judge(&mut born, &s, true);
    assert!(born.met && born.shortfall == 0.0, "{born:?}");
}

/// The lanternjaw's ladder: bank hatches and riders count apart from established hunters,
/// and a rider attached meets the 2-h target (0.5); a hatch alone does not.
#[test]
fn the_lanternjaw_is_held_to_progress_not_a_birth() {
    let mut s = stage();
    s.progress[Founder::Lanternjaw.index()] = true;
    let mut none = outcome(Founder::Lanternjaw);
    judge(&mut none, &s, true);
    assert_eq!(none.rule, Rule::LanternjawProgress);
    assert!(!none.met && none.shortfall == 1.0, "{none:?}");
    let mut hatched = outcome(Founder::Lanternjaw);
    hatched.bank_hatches = 2;
    hatched.alive_end = 2;
    judge(&mut hatched, &s, true);
    assert!(!hatched.met && hatched.score == 0.25, "{hatched:?}");
    let mut rider = outcome(Founder::Lanternjaw);
    rider.bank_hatches = 2;
    rider.riders_attached = 1;
    rider.riders_now = 1;
    rider.alive_end = 1;
    judge(&mut rider, &s, true);
    // Met at the 2-h pass line, but still graded above it (Astra F2): CMA sees the rest.
    assert!(rider.met && rider.score == 0.5 && rider.shortfall == 0.5, "{rider:?}");
    // Rider growth is progress inside 2 h (riders take 1-3 h to reach the drop-off line,
    // Fable 2026-10-02): a rider halfway there scores between attached and detached.
    let mut growing = rider.clone();
    growing.rider_progress = 0.5;
    judge(&mut growing, &s, true);
    assert!((growing.score - 0.625).abs() < 1e-12, "{growing:?}");
    let mut hunter = outcome(Founder::Lanternjaw);
    hunter.riders_attached = 1;
    hunter.rider_dropoffs = 1;
    hunter.free_living = 1;
    hunter.alive_end = 1;
    hunter.carrion_eaten = 0.1;
    // A juvenile off its host scavenging is not yet a hunter (the exhibit run's freed
    // riders ate carrion); a hit it lands makes it one.
    judge(&mut hunter, &s, true);
    assert_eq!(hunter.score, 0.75, "{hunter:?}");
    hunter.hits = 1;
    judge(&mut hunter, &s, true);
    assert_eq!(hunter.score, 1.0, "{hunter:?}");
}

/// The chorister: a litter (in either half), provisioned pups and pup survival; a litter
/// left unprovisioned falls short.
#[test]
fn the_chorister_is_held_to_its_pups() {
    let mut s = stage();
    s.progress[Founder::Chorister.index()] = true;
    let mut good = outcome(Founder::Chorister);
    good.alive_end = 7;
    good.litters = 1;
    good.pups_born = 4;
    good.pups_alive = 3;
    good.provision_received = 0.2;
    good.carrion_eaten = 0.5;
    good.intake_run = 0.3;
    judge(&mut good, &s, true);
    assert_eq!(good.rule, Rule::ChoristerProgress);
    assert!(good.met && good.shortfall == 0.0, "{good:?}");
    // Pups provisioned on the founders' endowment, but the pack never ate: not met.
    let mut hungry = good.clone();
    hungry.carrion_eaten = 0.0;
    hungry.intake_run = 0.0;
    judge(&mut hungry, &s, true);
    assert!(!hungry.met && (hungry.shortfall - 0.25).abs() < 1e-12, "{hungry:?}");
    // A hit without a meal is not intake (Astra F5).
    let mut hit_only = hungry.clone();
    hit_only.hits = 1;
    judge(&mut hit_only, &s, true);
    assert!(!hit_only.met, "{hit_only:?}");
    // Carrion bitten but nothing assimilated is not intake either.
    let mut bitten = hungry.clone();
    bitten.carrion_eaten = 0.5;
    judge(&mut bitten, &s, true);
    assert!(!bitten.met, "{bitten:?}");
    // The finalist check asks a pack kill and a meal, not carcass feeding alone.
    let mut f = s;
    f.chorister_kill_required = true;
    let mut scavenged = good.clone();
    judge(&mut scavenged, &f, true);
    assert!(!scavenged.met, "{scavenged:?}");
    scavenged.pack_kills = 1;
    judge(&mut scavenged, &f, true);
    assert!(scavenged.met, "{scavenged:?}");
    let mut unfed = good.clone();
    unfed.provision_received = 0.0;
    unfed.pups_alive = 1;
    judge(&mut unfed, &s, true);
    // Two of the four quarters missed (provisioning, survival).
    assert!(!unfed.met && (unfed.shortfall - 0.5).abs() < 1e-12, "{unfed:?}");
}

/// Replenishment is intake over a whole cycle's spending (intake minus the change in
/// biomass): a lineage that ate 1 and lost 0.25 spent 1.25 (0.8); one that ate 1 and
/// gained 0.1 spent 0.9 (over 1).
#[test]
fn replenishment_covers_a_whole_cycle() {
    assert!((replenishment(1.0, 2.0, 1.75) - 0.8).abs() < 1e-12);
    assert!(replenishment(1.0, 2.0, 2.1) > 1.0);
    assert_eq!(replenishment(0.0, 2.0, 1.0), 0.0);
    let mut s = stage();
    s.replenish[Founder::Browser.index()] = true;
    let mut o = outcome(Founder::Browser);
    o.alive_end = 8;
    o.late_births = 1;
    o.cycle_complete = true;
    o.intake_cycle = 1.0;
    o.biomass_start = 2.0;
    o.biomass_cycle = 1.75;
    judge(&mut o, &s, true);
    assert!((o.replenish_shortfall - 0.2).abs() < 1e-12, "{o:?}");
    // A world that stopped before a whole cycle has not shown it.
    o.cycle_complete = false;
    judge(&mut o, &s, true);
    assert_eq!(o.replenish_shortfall, 1.0);
}

// --- roster.toml ----------------------------------------------------------------------

fn roster_file() -> EcologyParams {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/training/search/roster.toml");
    EcologyParams::load(&path).expect("roster.toml loads")
}

/// x1 is the live terrarium: all nine lineages founded, glassfilm seeded, founders full.
#[test]
fn roster_decodes_over_the_terrarium() {
    let e = roster_file();
    let p = e.params().expect("decodes");
    assert!(p.fauna.founder_counts().iter().all(|n| *n > 0), "{:?}", p.fauna.founder_counts());
    assert!(p.flora.seed_film_at_founding);
    assert_eq!(p.fauna.founding.arrival_reserve, 1.0, "founders full (night's default)");
    // A saved genome carries its base: written and read back, it decodes the same.
    let text = e.genome().expect("a genome").to_toml("t");
    let back = EcologyParams::parse(&text).expect("a saved genome loads");
    assert_eq!(back.fauna(), e.fauna());
    assert_eq!(back.flora(VOXEL_M), e.flora(VOXEL_M));
}

/// The decode touches every listed path and nothing outside it: with every gene moved
/// at once, the edited leaves are exactly the brief's list plus its ties.
#[test]
fn roster_decode_touches_exactly_the_listed_paths() {
    use std::collections::BTreeSet;
    let e = roster_file();
    let mut g = e.genome().expect("a genome").clone();
    for gene in &mut g.genes {
        gene.value = match gene.mode {
            cubarium_voxel::genome::Mode::Scale => 1.37,
            cubarium_voxel::genome::Mode::Value => 0.7,
        };
    }
    let base = e.base_params().expect("the base");
    let flora_v = cubarium_voxel::registry::to_value(&base.flora).expect("flora");
    let fauna_v = cubarium_voxel::registry::to_value(&base.fauna).expect("fauna");
    let edits = g
        .resolve(&[
            (cubarium_voxel_flora::registry(), &flora_v),
            (cubarium_voxel_fauna::registry(), &fauna_v),
        ])
        .expect("resolves");
    let touched: BTreeSet<String> = edits.iter().flatten().map(|(p, _)| p.clone()).collect();

    let mut want = BTreeSet::new();
    for f in Founder::ALL {
        let at = format!("fauna.founders.{}", f.name());
        let core = &base.fauna.founder(f).core;
        for q in [
            "core.maintenance_per_s",
            "motor_respiration_per_s",
            "core.reserve_cap",
            "sleep_upkeep",
            "wake_hunger",
            "gut.stomach_k",
            "gut.bite_per_body_s",
            "gut.s0",
            "gut.refill_gap",
        ] {
            want.insert(format!("{at}.{q}"));
        }
        for food in cubarium_voxel_fauna::Food::ALL {
            want.insert(format!("{at}.yield_by_food.{}", food.name()));
        }
        // Digestion time per food class it eats (the minimal gut).
        for food in base.fauna.founder(f).diet.foods() {
            want.insert(format!("{at}.gut.digestion_s.{}", food.name()));
        }
        let _ = core;
    }
    want.insert("fauna.founders.chorister.pack.pack_size".into());
    // The parasite hatchling's knobs (lanternjaw recruitment, 2026-10-02; the take per
    // host body since the minimal gut).
    for q in ["parasite_take_per_day", "parasite_attach_radius_m", "parasite_detach_body_fraction"] {
        want.insert(format!("fauna.{q}"));
    }
    let group = |names: &[Species]| names.iter().map(|s| s.name()).collect::<Vec<_>>();
    use Species::*;
    for s in group(&[Springturf, Siphonreed, Velvetpad, Stonecushion, Bloomcrown, Umbrellafrond, Lanternberry, Vaulttree]) {
        for q in ["foliage_rate", "senescence", "alpha", "defence_per_bite", "defence_half_life_s"] {
            want.insert(format!("flora.{s}.{q}"));
        }
    }
    for s in Species::ALL {
        want.insert(format!("flora.{}.night_bloom_scale", s.name()));
    }
    for q in ["decomposition", "wood_decomposition", "carrion_decomposition"] {
        want.insert(format!("flora.{q}"));
    }
    let extra: Vec<&String> = touched.difference(&want).collect();
    let missing: Vec<&String> = want.difference(&touched).collect();
    assert!(extra.is_empty(), "outside the list: {extra:?}");
    assert!(missing.is_empty(), "listed but untouched: {missing:?}");
}

/// Fable 2026-10-02: life histories frozen (lifespan, growth, surplus floor, gestation),
/// motor tied to upkeep; per lineage upkeep, yield, reserve, sleep and wake hunger, and
/// since the minimal gut (repair 2 item 11) the stomach k, the bite b, the appetite
/// floor s0 and the refill gap (repair 3), with tau per food class each lineage eats (11:
/// the shredder and capgnawer eat two); the three parasite genes (the take per host
/// body), pack size, the 15 plant groups and the night bloom (night, 2c7fd076): 112. No
/// `intake`: the bite is its own.
#[test]
fn roster_genes_are_the_agreed_set() {
    let e = roster_file();
    let g = e.genome().expect("a genome");
    assert_eq!(g.genes.len(), 9 * 9 + 11 + 3 + 1 + 15 + 1, "{:?}", g.names());
    for frozen in ["lifespan", "maturity", "breed_reserve", "gestation", "motor", "intake"] {
        assert!(!g.names().iter().any(|n| n.starts_with(&format!("{frozen}@"))), "{frozen} is not a gene");
    }
    let base = e.base_params().expect("the base");
    for f in Founder::ALL {
        for kind in ["upkeep", "yield", "reserve", "sleep", "wake_hunger", "stomach", "bite", "s0", "refill_gap"] {
            let name = format!("{kind}@{}", f.name());
            assert!(g.names().iter().any(|n| *n == name), "{name}");
        }
        for food in base.fauna.founder(f).diet.foods() {
            let name = format!("tau_{}@{}", food.name(), f.name());
            assert!(g.names().iter().any(|n| *n == name), "{name}");
        }
    }
    assert!(g.names().iter().any(|n| n == "rider_take"));
}

/// The search ranges hold the design's defaults with room on both sides (the minimal
/// gut, repair 2 item 11): every multiplier's box holds x1 strictly inside, and the gut's
/// genes and upkeep reach x4 each way, so upkeep is centred on the gut's daily need f,
/// not on the old upkeep. Reserve per body still reaches x4.
#[test]
fn roster_ranges_hold_the_defaults_with_room() {
    let e = roster_file();
    let g = e.genome().expect("a genome");
    for gene in g.genes.iter().filter(|x| x.mode == cubarium_voxel::genome::Mode::Scale) {
        let (lo, hi) = gene.range;
        assert!(lo < 1.0 && hi > 1.0, "{}: [{lo}, {hi}]", gene.name);
    }
    for f in Founder::ALL {
        let range = |kind: &str| {
            let name = format!("{kind}@{}", f.name());
            g.genes.iter().find(|x| x.name == name).map(|x| x.range).expect("a gene")
        };
        for kind in ["upkeep", "stomach", "bite", "s0", "refill_gap"] {
            assert!(range(kind).0 <= 0.25 && range(kind).1 >= 4.0, "{kind}@{}", f.name());
        }
        assert!(range("reserve").1 >= 4.0, "{}: reserve to x4", f.name());
        let base = e.base_params().expect("the base");
        for food in base.fauna.founder(f).diet.foods() {
            let r = range(&format!("tau_{}", food.name()));
            assert!(r.0 <= 0.25 && r.1 >= 4.0, "tau_{}@{}", food.name(), f.name());
        }
    }
    let take = g.genes.iter().find(|x| x.name == "rider_take").expect("rider_take").range;
    assert!(take.0 <= 0.25 && take.1 >= 4.0, "{take:?}");
}

/// Every fauna leaf a roster gene moves stays inside its registry row's search interval at
/// both ends of the gene's box (the rows were widened where the gut's defaults needed it).
#[test]
fn roster_gene_boxes_sit_inside_the_registry_search_intervals() {
    let e = roster_file();
    let base = e.base_params().expect("the base");
    let fauna_v = cubarium_voxel::registry::to_value(&base.fauna).expect("fauna");
    let flora_v = cubarium_voxel::registry::to_value(&base.flora).expect("flora");
    let reg = cubarium_voxel_fauna::registry();
    for end in [0, 1] {
        let mut g = e.genome().expect("a genome").clone();
        for gene in &mut g.genes {
            gene.value = if end == 0 { gene.range.0 } else { gene.range.1 };
        }
        let edits = g
            .resolve(&[(cubarium_voxel_flora::registry(), &flora_v), (reg, &fauna_v)])
            .expect("resolves");
        for (path, v) in &edits[1] {
            let gut = path.contains(".gut.") || path.contains("maintenance_per_s") || path.contains("parasite_take");
            if !gut {
                continue;
            }
            let Some((lo, hi)) = reg.row_for(path).and_then(|r| r.search) else { continue };
            let v = v.as_float().or_else(|| v.as_integer().map(|i| i as f64)).expect("a number");
            assert!(v >= lo * (1.0 - 1e-9) && v <= hi * (1.0 + 1e-9), "{path} = {v} outside [{lo}, {hi}]");
        }
    }
}

/// **Study** (Fable 2026-10-02): latticevine faces per vine on the founded terrarium,
/// seeds 1–8, under roster.toml's base; the median sets `[screen.food] vine_faces_founded`.
/// `CUBARIUM_THREADS=1 cargo test -p cubarium-search --release vine_faces_founded -- --ignored --nocapture`
#[test]
#[ignore = "study: run by name"]
fn vine_faces_founded_on_the_terrarium() {
    let world = cubarium_voxel::world_config_from_file(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/desktop/terrarium.toml"),
    )
    .expect("the terrarium");
    let e = roster_file();
    let counts = e.fauna().founder_counts();
    let mut per_vine = Vec::new();
    for seed in 1..=8u64 {
        let (sim, info) = super::world::found_world(&world, seed, counts, &e);
        let cover = sim.flora().view().cover;
        let (vines, faces) = (cover.vines().len(), cover.faces().len());
        let x = faces as f64 / vines.max(1) as f64;
        println!("seed {seed} (world {}): {vines} vines, {faces} faces, {x:.2} per vine", info.seed);
        per_vine.push(x);
    }
    per_vine.sort_by(f64::total_cmp);
    println!("median faces per vine: {:.2}", 0.5 * (per_vine[3] + per_vine[4]));
}


// --- Astra's review (2026-10-02): the pilot's repairs -----------------------------

/// F2 / F11(b): a live, growing rider beats attach-and-die with eggs left, which earns no
/// attachment credit; a host-death release earns no juvenile credit; a free adult is
/// apart from a free juvenile.
#[test]
fn a_growing_rider_beats_attach_and_die() {
    let mut s = stage();
    s.progress[Founder::Lanternjaw.index()] = true;
    let mut dead = outcome(Founder::Lanternjaw);
    dead.bank_hatches = 2;
    dead.riders_attached = 2;
    dead.eggs_end = 5;
    judge(&mut dead, &s, true);
    assert_eq!(dead.score, 0.0, "attach-and-die: {dead:?}");
    assert!(!dead.met);
    let mut alive = outcome(Founder::Lanternjaw);
    alive.bank_hatches = 2;
    alive.riders_attached = 2;
    alive.riders_now = 2;
    alive.alive_end = 2;
    alive.eggs_end = 3;
    alive.rider_progress = 0.6;
    judge(&mut alive, &s, true);
    assert!(alive.met && alive.shortfall < dead.shortfall, "{alive:?} vs {dead:?}");
    // Half the riders dead: less than all alive at the same growth.
    let mut half = alive.clone();
    half.riders_now = 1;
    half.alive_end = 1;
    judge(&mut half, &s, true);
    assert!(half.score < alive.score && half.score > 0.5, "{half:?}");
    // Released by a dying host, no healthy drop-off: a live hatchling, no juvenile rung.
    let mut freed = outcome(Founder::Lanternjaw);
    freed.riders_attached = 1;
    freed.riders_freed = 1;
    freed.free_living = 1;
    freed.alive_end = 1;
    judge(&mut freed, &s, true);
    assert_eq!(freed.score, 0.25, "{freed:?}");
    // A healthy drop-off living free: the juvenile rung, growing toward birth size.
    let mut juvenile = freed.clone();
    juvenile.rider_dropoffs = 1;
    juvenile.free_progress = 0.5;
    judge(&mut juvenile, &s, true);
    assert!((juvenile.score - 0.85).abs() < 1e-12, "{juvenile:?}");
    // A free adult tops it.
    let mut adult = juvenile.clone();
    adult.free_adults = 1;
    judge(&mut adult, &s, true);
    assert_eq!(adult.score, 1.0, "{adult:?}");
}

/// F1 / F11(a): a readout-only lineage keeps its outcome but pays nothing, and its loss
/// neither counts as missing nor stops the world.
#[test]
fn a_readout_only_feeder_is_scored_by_nobody() {
    let mut s = stage();
    s.birth_required[Founder::Seedporter.index()] = true;
    s.replenish[Founder::Seedporter.index()] = true;
    s.readout_only[Founder::Seedporter.index()] = true;
    let mut o = outcome(Founder::Seedporter);
    o.cycle_complete = true;
    o.intake_cycle = 0.1;
    o.biomass_start = 3.0;
    judge(&mut o, &s, true);
    assert!(o.readout && !o.met, "still judged for the readout: {o:?}");
    assert_eq!((o.shortfall, o.replenish_shortfall), (0.0, 0.0), "{o:?}");
    let founded = vec![8, 8, 9, 6, 2, 8, 6, 4, 3];
    let mut bodies = vec![5usize; Founder::COUNT];
    bodies[Founder::Seedporter.index()] = 0;
    let eggs = vec![0usize; Founder::COUNT];
    assert_eq!(super::search::lost_lineages(&founded, &bodies, &eggs), 1);
    assert_eq!(super::search::scored_lost(&founded, &bodies, &eggs, &s.readout_only), 0);
    bodies[Founder::Browser.index()] = 0;
    assert_eq!(super::search::scored_lost(&founded, &bodies, &eggs, &s.readout_only), 1);
    // The band counts scored lineages only: the pilot's three feeders out.
    s.readout_only[Founder::Bellwing.index()] = true;
    s.readout_only[Founder::Chorister.index()] = true;
    let pop = [10usize; Founder::COUNT];
    assert_eq!(super::search::scored_animals(&pop, &s.readout_only), 60);
}

/// F3: food access refuses only where every supply path is absent; a short but real
/// supply runs and pays a soft term.
#[test]
fn food_short_of_need_is_soft_and_only_zero_supply_refuses() {
    let w = world();
    let mut s = spec();
    s.night = None;
    let fauna = fauna_with(&[(Founder::Seedporter, 4)]);
    // Short but real: at the gut's daily need the terrarium's fruit feeds four ~140x
    // over, so every fruit source is cut to a thousandth.
    let mut short = flora();
    for sp in Species::ALL {
        short.species_mut(sp).fruit_per_s *= 1e-3;
    }
    short.latticevine.fruit *= 1e-3;
    let r = roster(&short, &fauna, &w, &s);
    let row = r.food.iter().find(|x| x.lineage == "seedporter").expect("row");
    assert!(row.ratio > 0.0 && row.ratio < 1.0, "{row:?}");
    assert!(r.refusals().is_empty(), "{:?}", r.refusals());
    assert!(r.soft(5.0) > 0.0);
    let mut none = flora();
    for sp in Species::ALL {
        none.species_mut(sp).fruit_per_s = 0.0;
    }
    none.latticevine.fruit = 0.0;
    let r = roster(&none, &fauna, &w, &s);
    assert_eq!(refusals_naming(&r, "food", "seedporter").len(), 1);
}

fn pilot_file() -> EcologyParams {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/training/search/roster-pilot.toml");
    EcologyParams::load(&path).expect("roster-pilot.toml loads")
}

/// F1, F7, F10 / F11(c): the pilot file reaches the intended score and serialization
/// path: 60 searched genes since the minimal gut (four searched lineages x upkeep, yield,
/// reserve, sleep, wake hunger, stomach, bite, s0 and refill gap, their 6 tau genes, and
/// 18 plant and rider genes; the five readout-only lineages' genes and pack size frozen), the
/// feeders readout-only, R a readout, S and F soft, the night envelope hard, food soft;
/// it round-trips through a saved genome; and the launch script runs it with the agreed
/// settings.
#[test]
fn the_pilot_launch_is_the_agreed_experiment() {
    let e = pilot_file();
    let g = e.genome().expect("a genome");
    assert_eq!(g.search_bounds().len(), 60, "searched: {}", g.genes.iter().filter(|x| !x.frozen).count());
    assert_eq!(g.to_search().len(), 60);
    let base = e.base_params().expect("base");
    for f in [Founder::Bellwing, Founder::Seedporter, Founder::Chorister, Founder::Lanternjaw, Founder::Loftstrider] {
        let mut kinds: Vec<String> = ["upkeep", "yield", "reserve", "sleep", "wake_hunger", "stomach", "bite", "s0", "refill_gap"]
            .iter()
            .map(|k| k.to_string())
            .collect();
        kinds.extend(base.fauna.founder(f).diet.foods().map(|food| format!("tau_{}", food.name())));
        for kind in kinds {
            let name = format!("{kind}@{}", f.name());
            let gene = g.genes.iter().find(|x| x.name == name).unwrap_or_else(|| panic!("{name}"));
            assert!(gene.frozen, "{name} is frozen");
        }
    }
    assert!(!g.names().iter().any(|n| n.starts_with("intake@")), "no intake gene: the bite is its own");
    assert!(g.genes.iter().find(|x| x.name == "pack_size@chorister").expect("pack").frozen);
    // Upkeep is x1 at the gut's daily need (the exhibit's cuts are gone); a pack of three.
    let p = e.params().expect("decodes");
    let ch = |q: &cubarium_voxel_fauna::FaunaConfig| q.founder(Founder::Chorister).core.maintenance_per_s;
    assert!((ch(&p.fauna) / ch(&base.fauna) - 1.0).abs() < 1e-12);
    assert_eq!(p.fauna.founder(Founder::Chorister).pack.expect("pack").pack_size, 3);
    // Targets: the three feeders readout-only, and since the balance addendum the
    // lanternjaw and loftstrider too; the other four scored on balance, counts readouts.
    let band = super::search::BandSpec::default().with_targets(&g.rest).expect("targets");
    let st = band.stage.expect("v3");
    for f in Founder::ALL {
        let readout = matches!(
            f,
            Founder::Bellwing | Founder::Seedporter | Founder::Chorister | Founder::Lanternjaw | Founder::Loftstrider
        );
        assert_eq!(st.readout_only[f.index()], readout, "{}", f.name());
        assert_eq!(st.balance[f.index()], !readout, "{}", f.name());
    }
    assert!(st.counts_readout && st.balance_weight > 0.0);
    // Screens: R a readout, S/F soft (every miss runs), the night envelope hard.
    let sc = ScreenSpec::from_rest(&g.rest).expect("[screen]");
    assert!(sc.r_readout && sc.night.is_some() && sc.food.is_some());
    let mut rsf = super::screen::rsf(&flora(), &FaunaConfig::default(), VOXEL_M, &sc);
    (rsf.r, rsf.s, rsf.f) = (0.5, 50.0, 0.01);
    assert_eq!(sc.verdict(&rsf), super::screen::Verdict::Margin, "S/F misses run");
    assert!(sc.soft(&rsf) > 0.0);
    // A saved nominee keeps its frozen genes frozen and decodes the same.
    let back = EcologyParams::parse(&g.to_toml("t")).expect("a saved genome");
    assert_eq!(back.genome().expect("g").search_bounds().len(), 60);
    assert_eq!(back.fauna(), e.fauna());
    // The launch script.
    let sh = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/training/search/roster-pilot.sh"),
    )
    .expect("the launch script");
    for want in [
        "--search config/training/search/roster-pilot.toml",
        "--world config/desktop/terrarium.toml",
        "LAMBDA:-8",
        "SEEDS:-3",
        "HOURS:-2",
        "--lambda \"$LAMBDA\"",
        "--seeds-per-generation \"$SEEDS\"",
        "--hours \"$HOURS\"",
        "--sigma0 0.08",
        "--water-every 1",
        "CUBARIUM_THREADS=1",
        "nice -n 10",
        "ulimit -v",
    ] {
        assert!(sh.contains(want), "roster-pilot.sh lacks `{want}`");
    }
}


/// Astra F4: a birth with a negative cycle balance meets the stage, not replenishment,
/// and so not the combined target; the three verdicts are kept apart.
#[test]
fn stage_replenishment_and_combined_verdicts_are_apart() {
    let mut s = stage();
    s.birth_required[Founder::Browser.index()] = true;
    s.replenish[Founder::Browser.index()] = true;
    let mut o = outcome(Founder::Browser);
    o.alive_end = 9;
    o.late_births = 1;
    o.cycle_complete = true;
    o.intake_cycle = 1.0;
    o.biomass_start = 2.0;
    o.biomass_cycle = 1.75;
    judge(&mut o, &s, true);
    assert!(o.stage_met && !o.replenishment_met && !o.met, "{o:?}");
    o.biomass_cycle = 2.1;
    judge(&mut o, &s, true);
    assert!(o.stage_met && o.replenishment_met && o.met, "{o:?}");
    // An incomplete cycle has not shown replenishment.
    o.cycle_complete = false;
    judge(&mut o, &s, true);
    assert!(!o.replenishment_met && !o.met, "{o:?}");
    // Not held to replenishment: the stage alone.
    s.replenish[Founder::Browser.index()] = false;
    judge(&mut o, &s, true);
    assert!(o.replenishment_met && o.met, "{o:?}");
}

