//! The animal layer's parameter table (`fauna.*`, package R1): one row per numeric field of
//! [`crate::FaunaConfig`], mounted where its struct sits — the placeholder species
//! (`fauna.frondgrazer`) and each founder's core (`fauna.founders.<lineage>.core`) carry
//! the same [`crate::SpeciesConfig`] rows. Units, grades, provenance and search ranges are
//! the audit's `parameters.tsv` where it has a row; P1's predation numbers have none and
//! are `placeholder`. The generic code is [`cubarium_voxel::registry`].

use cubarium_voxel::registry::{Alias, Range, Registry, Row};

use crate::{BirthMode, FaunaConfig, Food, Founder, SpeciesConfig};

/// The animal layer's table.
pub fn registry() -> &'static Registry {
    static TABLE: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let founders: Vec<&'static str> = Founder::ALL.iter().map(|f| f.name()).collect();
        let foods: Vec<&'static str> = Food::ALL.iter().map(|f| f.name()).collect();
        Registry::new(
            "fauna",
            rows(),
            vec![
                Alias { pattern: "fauna.founders".into(), names: founders.clone() },
                Alias { pattern: "fauna.founders.*.yield_by_food".into(), names: foods },
                Alias { pattern: "fauna.founders.*.hunter.prey".into(), names: founders },
            ],
            Vec::new(),
        )
    })
}

fn r(path: &str, unit: &'static str, range: Range, what: &'static str) -> Row {
    Row::new(path.to_string(), unit, range, what)
}

/// [`SpeciesConfig`]'s rows, relative to where it is mounted.
fn species_rows() -> Vec<Row> {
    let nonneg = Range::nonneg();
    vec![
        r("maintenance_per_s", "org-units/s per unit body", nonneg, "basal upkeep, paid from reserve then body")
            .search(2.5e-4, 4e-3).log().grade("B"),
        r("bite_per_s", "org-units/s", nonneg, "the hungry mouth's intake rate")
            .search(2e-4, 4e-3).log().grade("C"),
        r("bite_half_stock", "org-units at the mouth", nonneg, "K: a bite is want * E / (E + K)")
            .search(1e-4, 5e-2).log().grade("C").prov("tuned"),
        r("n_tissue", "mineral per org-unit", nonneg, "tissue stoichiometry: a bite builds at most mineral / n_tissue")
            .search(0.01, 0.12).log().grade("A"),
        r("body_max", "org-units", nonneg, "adult structure").grade("D"),
        r("growth_max_per_s", "org-units/s", nonneg, "fastest structure grows from the reserve")
            .search(1e-6, 1e-4).log().grade("B").prov("derived"),
        r("body_min", "org-units", Range::positive(), "death threshold, and a newborn's body").evolve().grade("B"),
        r("birth_body", "org-units", nonneg, "structure at which it can pay for a birth").evolve().grade("B"),
        r("birth_cost", "org-units", nonneg, "a gestation's package: the newborn's body_min plus its reserve")
            .evolve().grade("B"),
        r("reserve_cap", "fraction of body", nonneg, "ceiling on new intake into the reserve")
            .search(0.2, 1.5).grade("A"),
        r("drown_depth_m", "m", nonneg, "standing water that kills the heuristic body and a clutch").grade("D"),
        r("energy_density", "energy per org-unit", nonneg, "energy an introduced animal arrives with").grade("N"),
        r("lifespan_median_s", "s", nonneg, "median lifespan (0: none)")
            .search(3600.0, 57600.0).log().grade("B").prov("derived"),
        r("reproduction.surplus_floor", "org-units above the package", nonneg, "reserve margin above the package that counts as surplus")
            .evolve().grade("C"),
        r("reproduction.surplus_hold_s", "s", nonneg, "the window a surplus is judged over")
            .search(60.0, 3600.0).log().grade("C").prov("derived"),
        r("reproduction.surplus_share", "fraction of the hold", Range::fraction(), "share of the hold spent in surplus before breeding")
            .prov("decision"),
        r("reproduction.gestation_s", "s", nonneg, "a gestation's length").search(60.0, 6000.0).log().grade("D"),
        r("reproduction.gestation_loss_fraction", "fraction", Range::fraction(), "organic respired when a gestation fails")
            .search(0.0, 1.0).grade("C"),
        r("reproduction.birth_interval_s", "s", nonneg, "refractory after a birth or a laying")
            .search(300.0, 10000.0).log().grade("B").prov("derived"),
        r("reproduction.clutch_size", "eggs", nonneg, "eggs per clutch (an egg-layer's at least 1)")
            .evolve().grade("D").prov("derived"),
        r("reproduction.egg_organic", "org-units per egg", nonneg, "one egg; at least the lineage's body_min")
            .evolve().grade("D"),
        r("reproduction.incubation_s", "s", nonneg, "a clutch incubates this long").search(60.0, 6000.0).log().grade("B"),
    ]
}

/// [`crate::FounderPhysiology`]'s own rows, relative to `fauna.founders.<lineage>`.
fn founder_rows() -> Vec<Row> {
    let nonneg = Range::nonneg();
    let fraction = Range::fraction();
    vec![
        r("motor_respiration_per_s", "org-units/s per unit body at full cruise", nonneg, "paid motion")
            .search(2.5e-4, 1e-2).log().grade("B"),
        r("step_up_m", "m", nonneg, "tallest riser stepped (rounded down to voxels)")
            .search(0.1, 1.0).grade("B").prov("measured"),
        r("step_down_m", "m", nonneg, "deepest drop stepped").evolve().grade("C").prov("measured"),
        r("wall_climb_cost_factor", "x walking cost", nonneg, "motor cost of a second on a wall").prov("derived"),
        r("wade_height_fraction", "x current height", nonneg, "deepest water walked into").search(0.0, 1.0).grade("B"),
        r("drown_height_fraction", "x current height", nonneg, "water deeper than this is over it").grade("A").prov("decision"),
        r("drown_after_s", "s", nonneg, "continuous time under water before it drowns")
            .search(10.0, 36000.0).log(),
        r("adult_length_m", "m", Range::positive(), "adult body length (cruise is 1 BL/s)")
            .evolve().digest().grade("N").prov("decision"),
        r("adult_width_m", "m", Range::positive(), "adult body width").evolve().digest().grade("N").prov("decision"),
        r("adult_height_m", "m", Range::positive(), "adult body height").evolve().digest().grade("N").prov("decision"),
        r("eye_height_fraction", "x body height", nonneg, "the eye over the standing surface").digest(),
        r("mouth_ceiling_fraction", "x body height", nonneg, "the mouth band's ceiling")
            .search(0.3, 2.5).digest(),
        r("contact_height_fraction", "x body height", nonneg, "the contact receptors' height").digest(),
        r("mouth_reach_length_fraction", "x body length", nonneg, "the mouth's reach ahead of the footprint")
            .evolve().digest(),
        r("yield_by_food.*", "fraction", fraction, "assimilation yield of one food class").search(0.05, 0.9),
        r("organ_structure_fraction", "fraction of body", Range::closed(0.0, 1.0).open_hi(), "sensor/organ share of structure")
            .grade("N"),
        r("sleep_upkeep", "x basal upkeep", fraction, "upkeep while asleep").search(0.2, 1.0).prov("decision"),
        r("wake_hunger", "hunger 0..1", fraction, "hunger at or above which it stays awake").search(0.1, 1.0),
        r("armour", "fraction", fraction, "a hit is reduced by 1 - armour"),
        r("agility", "fraction", fraction, "the dodge's species factor"),
        r("mass_scale", "x placeholder mass", Range::positive(), "the org-unit mass scale the body was built at")
            .prov("decision"),
        // The carnivore's table (`Hunter`): the lanternjaw's only, optional on the others.
        r("hunter.strike_power", "wound per hit", nonneg, "a hit adds power x (S_pred / S_prey) x (1 - armour)").optional(),
        r("hunter.prey_max_ratio", "x own body", nonneg, "the size window: prey at most this x its own").optional(),
        r("hunter.strike_reach_length_fraction", "x body length", nonneg, "the strike capsule's reach").optional(),
        r("hunter.jaw_band_height_fraction", "x body height", nonneg, "the jaw band's top").optional(),
        r("hunter.windup_s", "s", nonneg, "the crouch").optional(),
        r("hunter.lunge_s", "s", nonneg, "the lunge").optional(),
        r("hunter.recovery_s", "s", nonneg, "the recovery, at half speed and unable to strike").optional(),
        r("hunter.strike_cost_s", "s of awake upkeep", nonneg, "a strike's cost, hit or miss").optional(),
        r("hunter.lure_power", "x awake upkeep", nonneg, "a lit lure's cost").optional(),
        r("hunter.rest_at", "fraction of reserve cap", fraction, "satiated: rests at this reserve").optional(),
        r("hunter.torpor_upkeep", "x basal upkeep", nonneg, "upkeep in the satiated rest").optional(),
        r("hunter.wake_at", "fraction of reserve cap", fraction, "wakes from the rest below this").optional(),
        // The counter-strike seam (`Counter`): unused, `None` for every lineage.
        r("counter.reach", "m", nonneg, "counter-strike reach (unused seam)").optional(),
        r("counter.damage", "wound", nonneg, "counter-strike damage (unused seam)").optional(),
        r("counter.cost", "org-units", nonneg, "counter-strike cost (unused seam)").optional(),
    ]
}

fn rows() -> Vec<Row> {
    let mut rows: Vec<Row> = species_rows().into_iter().map(|row| row.under("fauna.frondgrazer")).collect();
    for f in Founder::ALL {
        let at = format!("fauna.founders.{}", f.name());
        rows.extend(species_rows().into_iter().map(|row| row.under(&format!("{at}.core"))));
        rows.extend(founder_rows().into_iter().map(|row| row.under(&at)));
    }
    rows.extend([
        r("fauna.hatch_chance_per_dusk", "probability per dusk", Range::fraction(), "a dormant egg past its footfall gate hatches at one dusk"),
        r("fauna.parasite_attach_radius_m", "m", Range::nonneg(), "host search radius from a hatching egg").search(0.25, 2.0).grade("N"),
        r("fauna.parasite_max_per_host", "count", Range::nonneg(), "riders one host carries (0: dormant eggs never hatch)")
            .search(0.0, 4.0).grade("N"),
        r("fauna.parasite_drain_of_host_basal", "x host basal upkeep", Range::nonneg(), "a rider's take from its host")
            .search(0.01, 0.5).log().grade("N"),
        r("fauna.parasite_detach_body_fraction", "fraction of lineage body_max", Range::closed(0.0, 1.0).open_lo(), "a rider drops off at this size")
            .search(0.15, 0.5).grade("N"),
    ]);
    rows
}

/// The cross-field rules no single row can state: the body thresholds in an order that
/// builds a living animal, and an egg-layer's clutch that can build a hatchling.
pub(crate) fn cross_rules(c: &FaunaConfig) -> Result<(), String> {
    species_rules("fauna.frondgrazer", &c.frondgrazer)?;
    for f in Founder::ALL {
        species_rules(&format!("fauna.founders.{}.core", f.name()), &c.founder(f).core)?;
    }
    Ok(())
}

fn species_rules(at: &str, sc: &SpeciesConfig) -> Result<(), String> {
    if sc.body_min > sc.birth_body || sc.birth_body > sc.body_max {
        return Err(format!(
            "{at} needs body_min <= birth_body <= body_max, not {} <= {} <= {}",
            sc.body_min, sc.birth_body, sc.body_max
        ));
    }
    if sc.birth_cost < sc.body_min {
        return Err(format!(
            "{at}.birth_cost {} cannot build a newborn of body_min {}",
            sc.birth_cost, sc.body_min
        ));
    }
    let r = &sc.reproduction;
    if r.mode == BirthMode::Eggs {
        if r.clutch_size == 0 {
            return Err(format!("{at}.reproduction.clutch_size must be at least one egg"));
        }
        if r.egg_organic < sc.body_min {
            return Err(format!(
                "{at}.reproduction.egg_organic {} cannot build a hatchling of body_min {}",
                r.egg_organic, sc.body_min
            ));
        }
    }
    Ok(())
}
