//! The animal layer's parameter table (`fauna.*`, package R1): one row per numeric field of
//! [`crate::FaunaConfig`], mounted where its struct sits — the placeholder species
//! (`fauna.frondgrazer`) and each founder's core (`fauna.founders.<lineage>.core`) carry
//! the same [`crate::SpeciesConfig`] rows. Units, grades, provenance and search ranges are
//! the audit's `parameters.tsv` where it has a row; P1's predation numbers have none and
//! are `placeholder`. The generic code is [`cubarium_voxel::registry`].

use cubarium_voxel::registry::{Alias, Group, GroupKind, Range, Registry, Row, Tie};

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
        .with_ties(ties())
        .with_groups(groups())
    })
}

/// The eight absolute quantities a body's org-unit mass scale carries
/// ([`crate::SpeciesConfig::scale_mass`]), relative to a `SpeciesConfig`. Not
/// `bite_half_stock`: it is in the food's stock units, and the food is not scaled.
pub const MASS_SCALED: [&str; 8] = [
    "body_max",
    "body_min",
    "birth_body",
    "birth_cost",
    "reproduction.surplus_floor",
    "reproduction.egg_organic",
    "bite_per_s",
    "growth_max_per_s",
];

/// The animal layer's `derive()` (registry §4): the fields that follow others by ratio
/// when a genome moves the others.
/// - Walking is priced against upkeep (audit §8.1): `motor_respiration_per_s =
///   motor_factor × maintenance_per_s`, the factor today's ratio.
/// - The mouth too: `bite_per_s = intake_factor × maintenance_per_s × body`.
/// - The eight absolute quantities follow the lineage's mass scale (audit §1.4); the bite's
///   half-saturation, in food units, does not.
/// - The placeholder species is the browser's body: its mass and lifespan follow the
///   browser's.
pub fn ties() -> Vec<Tie> {
    let mut t = vec![
        Tie::new(
            "motor_follows_upkeep",
            "fauna.founders.*.motor_respiration_per_s",
            "fauna.founders.*.core.maintenance_per_s",
        ),
        Tie::new(
            "intake_follows_upkeep",
            "fauna.founders.*.core.bite_per_s",
            "fauna.founders.*.core.maintenance_per_s",
        ),
    ];
    for q in MASS_SCALED {
        t.push(Tie::new("mass", format!("fauna.founders.*.core.{q}"), "fauna.founders.*.mass_scale"));
        t.push(Tie::new(
            "placeholder_mass",
            format!("fauna.frondgrazer.{q}"),
            "fauna.founders.frondgrazer.mass_scale",
        ));
    }
    t.push(Tie::new(
        "placeholder_lifespan",
        "fauna.frondgrazer.lifespan_median_s",
        "fauna.founders.frondgrazer.core.lifespan_median_s",
    ));
    t
}

/// The animal layer's share of the biological clock (`K_bio`, registry decision 2):
/// `@bio_rate` runs faster with it, `@bio_time` shorter. Derived fields are left out —
/// they follow their bases.
fn groups() -> Vec<Group> {
    let core = |q: &str| format!("fauna.founders.*.core.{q}");
    vec![
        Group {
            name: "bio_rate",
            kind: GroupKind::Paths(vec![
                core("maintenance_per_s"),
                core("growth_max_per_s"),
                "fauna.predation.wound_heal_per_s".into(),
                "fauna.predation.egg_upkeep_per_s".into(),
            ]),
        },
        Group {
            name: "bio_time",
            kind: GroupKind::Paths(vec![
                core("lifespan_median_s"),
                core("reproduction.surplus_hold_s"),
                core("reproduction.gestation_s"),
                core("reproduction.birth_interval_s"),
                core("reproduction.incubation_s"),
                "fauna.predation.egg_viability_median_s".into(),
            ]),
        },
    ]
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
        r("reproduction.litter_min", "organic units", nonneg, "litter an egg-layer's face must hold (more than this) for it to lay")
            .search(0.0, 0.2),
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
            .search(0.1, 1.0).digest().grade("B").prov("measured"),
        r("step_down_m", "m", nonneg, "deepest drop stepped").evolve().digest().grade("C").prov("measured"),
        r("wall_climb_cost_factor", "x walking cost", nonneg, "motor cost of a second on a wall").prov("derived"),
        r("climb_speed_fraction", "x cruise", Range::positive(), "pace of a wall climb, a fraction of the cruise")
            .search(0.1, 1.0).grade("B").prov("decision"),
        r("cruise_body_lengths_per_s", "body lengths/s", Range::positive(), "full-forward pace (cruise = this x adult length)")
            .search(0.3, 3.0).digest().grade("A").prov("decision"),
        r("yaw_cap_rad_per_s", "rad/s", Range::positive(), "the turn rate of full turn effort")
            .search(1.0, 10.0).digest().grade("B"),
        r("wade_height_fraction", "x current height", nonneg, "deepest water walked into").search(0.0, 1.0).digest().grade("B"),
        r("drown_height_fraction", "x current height", nonneg, "water deeper than this is over it").digest().grade("A").prov("decision"),
        r("drown_after_s", "s", nonneg, "continuous time under water before it drowns")
            .search(10.0, 36000.0).log().digest(),
        r("adult_length_m", "m", Range::positive(), "adult body length (cruise is 1 BL/s)")
            .evolve().digest().grade("N").prov("decision"),
        r("adult_width_m", "m", Range::positive(), "adult body width").evolve().digest().grade("N").prov("decision"),
        r("adult_height_m", "m", Range::positive(), "adult body height").evolve().digest().grade("N").prov("decision"),
        r("eye_height_fraction", "x body height", nonneg, "the eye over the standing surface").digest(),
        r("mouth_ceiling_fraction", "x body height", nonneg, "the mouth band's ceiling")
            .search(0.3, 2.5).digest(),
        r("mouth_floor_fraction", "x body height", nonneg, "the mouth band's floor (ground pools are taken whatever it is)")
            .prov("decision"),
        r("bite_spill_fraction", "fraction of a bite", fraction, "share of each bite's withdrawn tissue spilt as litter at the standing face")
            .prov("decision"),
        r("founding_count", "bodies (clutches for an egg bank)", nonneg, "how many a fresh world founds (a new lineage's default 0)")
            .prov("decision"),
        r("water_margin_m", "m", nonneg, "an aquatic body's step needs standing water within this of its destination face")
            .prov("decision"),
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
            .digest().prov("decision"),
        r("taste_reference", "organic units per site", Range::positive(), "food stock per site that tastes as a cue of 1 before saturation (absent: emit_reference x mass_scale)")
            .optional().digest().prov("decision"),
        // The carnivore's table (`Hunter`): the lanternjaw's only, optional on the others.
        r("hunter.strike_power", "wound per hit", nonneg, "a hit adds power x (S_pred / S_prey) x (1 - armour)").optional(),
        r("hunter.prey_max_ratio", "x own body", nonneg, "the size window: prey at most this x its own").optional(),
        r("hunter.strike_reach_length_fraction", "x body length", nonneg, "the strike capsule's reach").optional(),
        r("hunter.jaw_band_height_fraction", "x body height", nonneg, "the jaw band's top").optional(),
        r("hunter.jaw_forward_length_fraction", "x body length", nonneg, "the jaw's point ahead of the body's centre").search(0.2, 0.8).optional(),
        r("hunter.windup_s", "s", nonneg, "the crouch").optional(),
        r("hunter.lunge_s", "s", nonneg, "the lunge").optional(),
        r("hunter.recovery_s", "s", nonneg, "the recovery, at half speed and unable to strike").optional(),
        r("hunter.strike_cost_s", "s of awake upkeep", nonneg, "a strike's cost, hit or miss").optional(),
        r("hunter.lure_power", "x awake upkeep", nonneg, "a lit lure's cost").optional(),
        r("hunter.rest_at", "fraction of reserve cap", fraction, "satiated: rests at this reserve").optional(),
        r("hunter.torpor_upkeep", "x basal upkeep", nonneg, "upkeep in the satiated rest").optional(),
        r("hunter.wake_at", "fraction of reserve cap", fraction, "wakes from the rest below this").optional(),
        // The flyer's table (`Flight`, F1): the bellwing's only, optional on the others.
        r("flight.span_m", "m", Range::positive(), "wingspan: the collision sphere's diameter").optional().prov("decision"),
        r("flight.band_max_m", "m", nonneg, "the ceiling: the base's most height over the support face below").optional().prov("decision"),
        r("flight.flight_factor", "x basal upkeep", nonneg, "a second airborne at cruise").optional().prov("decision"),
        r("flight.hover_factor", "x basal upkeep", nonneg, "a second hovering").optional().prov("decision"),
        r("flight.climb_work_per_m", "org-units per m per unit body", nonneg, "lift work going up; descent is free").optional(),
        r("flight.takeoff_s", "s of flight", nonneg, "a takeoff's cost").optional().prov("decision"),
        r("flight.foliage_speed_factor", "x speed cap", fraction, "the speed cap inside a crown's foliage layers").optional().prov("decision"),
        r("flight.shower_rain_reference_m_per_s", "m/s of rain", Range::positive(), "R0: flight costs x (1 + rain rate / R0) in a shower").optional(),
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
    rows.extend(knob_rows());
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

/// The rows of [`crate::FaunaConfig`]'s grouped knobs (`predation`, `life`, `cue`,
/// `teachers`, `founding`; package R2-fauna): units and meanings from the field docs,
/// ranges the smallest that keep the rule defined, search intervals plausible bounds
/// around today's value (`placeholder` unless the design note pins the number).
fn knob_rows() -> Vec<Row> {
    let nonneg = Range::nonneg();
    let pos = Range::positive();
    let fraction = Range::fraction();
    let mut rows = Vec::new();

    // `fauna.predation.*`: predation-and-flight §2.
    let p = |path: &str, unit: &'static str, range: Range, what: &'static str| {
        r(&format!("fauna.predation.{path}"), unit, range, what)
    };
    rows.extend([
        p("still_after_s", "s", nonneg, "a body is still once it has not moved this long").search(0.5, 10.0).prov("decision"),
        p("still_speed_bl_per_s", "body lengths/s", nonneg, "ground speed that counts as moving").search(0.02, 0.5).prov("decision"),
        p("still_threat_near_m", "m", nonneg, "a still threat reads as a threat inside this distance").search(0.1, 2.0).prov("decision"),
        p("tremor_radius_m", "m", nonneg, "tremor is summed over bodies within this radius").search(0.5, 4.0).prov("decision"),
        p("footfall_radius_m", "m", nonneg, "an egg hears prey that pass within this radius").search(0.5, 5.0).prov("decision"),
        p("hatch_suppress_m", "m", nonneg, "an adult of the egg's lineage this near suppresses its hatching").search(1.0, 10.0).prov("decision"),
        p("footfall_tremor_min", "organic x m/s / m^2", nonneg, "the tremor a footstep must make at an egg to be heard")
            .search(1e-5, 1e-2).log(),
        p("tremor_near_m", "m", pos, "distances below this are read as this in the tremor's 1/d^2"),
        p("awareness_walking", "fraction", fraction, "dodge table awareness, walking").prov("decision"),
        p("awareness_resting", "fraction", fraction, "dodge table awareness, awake resting").prov("decision"),
        p("awareness_cropping", "fraction", fraction, "dodge table awareness, cropping and climbing").prov("decision"),
        p("awareness_asleep", "fraction", fraction, "dodge table awareness, asleep").prov("decision"),
        p("dodge_rear_aspect", "fraction", fraction, "dodge aspect factor when struck from the rear half").search(0.1, 1.0).prov("decision"),
        p("dodge_condition_floor", "fraction", fraction, "the dodge's condition factor is the reserve fraction clamped to [this, 1]").search(0.1, 1.0).prov("decision"),
        p("egg_footfall_min", "distinct prey", nonneg, "lowest footfall threshold a dormant egg draws").search(1.0, 6.0),
        p("egg_footfall_max", "distinct prey", nonneg, "highest footfall threshold a dormant egg draws").search(4.0, 20.0),
        p("egg_viability_median_s", "s", pos, "median viability of a dormant egg").search(3600.0, 172800.0).log(),
        p("egg_viability_spread", "factor", Range::at_least(1.0), "the viability draw's multiplicative spread each way").search(1.0, 4.0),
        p("wound_heal_per_s", "wound/s", nonneg, "the rate a wound heals (0.5 per half hour)").search(1e-5, 1e-2).log().prov("decision"),
        p("heal_cost", "organic per wound per body", nonneg, "organic matter a unit of wound costs to heal, per unit of body").search(0.0, 5.0),
        p("egg_upkeep_per_s", "per s", nonneg, "what a dormant egg respires per unit of its own organic matter").search(1e-7, 1e-4).log(),
        p("alert_s", "s", nonneg, "how long a hit keeps its prey awake").search(0.0, 600.0),
        p("wound_slow", "per wound", fraction, "speed is multiplied by 1 - this x wound").search(0.0, 1.0).prov("decision"),
        p("recovery_speed", "x speed", fraction, "a strike's recovery is walked at this fraction of the speed").search(0.1, 1.0).prov("decision"),
        p("flee_sight_m", "m", nonneg, "how far a sighted prey heuristic looks for a threat").search(1.0, 8.0).prov("decision"),
        p("flee_tremor", "tremor units", nonneg, "the tremor a blind prey heuristic flees").search(1e-3, 0.1).log(),
    ]);

    // `fauna.life.*`.
    let l = |path: &str, unit: &'static str, range: Range, what: &'static str| {
        r(&format!("fauna.life.{path}"), unit, range, what)
    };
    rows.extend([
        l("day_line", "daylight 0..1", fraction, "daylight a diurnal body wakes above and a nocturnal one sleeps above")
            .search(0.05, 0.6).grade("C"),
        l("twilight_lo", "daylight 0..1", fraction, "lower edge of the twilight band (crepuscular rhythm, unused today)").grade("N"),
        l("twilight_hi", "daylight 0..1", fraction, "upper edge of the twilight band (crepuscular rhythm, unused today)").grade("N"),
        l("gut_void_s", "s", pos, "how often an animal voids its gut onto its own face")
            .search(5.0, 600.0).log().grade("C"),
        l("lifespan_draw_min", "x median", pos, "lowest multiple of the median a lifespan draw takes").search(0.5, 1.0).grade("C"),
        l("lifespan_draw_span", "x median", nonneg, "width of the lifespan draw, in medians").search(0.0, 1.0).grade("C"),
        l("arriving_share_min", "fraction", fraction, "an arriving body keeps a uniform [this, 1) share of a draw").grade("N"),
    ]);

    // `fauna.cue.*`.
    let c = |path: &str, unit: &'static str, range: Range, what: &'static str| {
        r(&format!("fauna.cue.{path}"), unit, range, what)
    };
    rows.extend([
        c("emit_reference", "organic units", pos, "detritus stock at which a source emits at full strength; the taste reference").grade("C"),
        c("half_life_s", "s", pos, "cue half-life").search(0.5, 60.0).log().grade("C"),
        c("diffuse_fraction", "fraction per update", fraction, "convex nearest-neighbour diffusion mix").search(0.05, 0.9).grade("C"),
        c("threshold", "cue units", nonneg, "cue values below this are dropped after an update").grade("N"),
        c("pool_bulk_density", "organic units/m^3", pos, "bulk density a ground pool's physical height is computed with")
            .digest().grade("D"),
    ]);

    // `fauna.teachers.*`: the heuristic controllers' reflexes.
    let t = |path: &str, unit: &'static str, range: Range, what: &'static str| {
        r(&format!("fauna.teachers.{path}"), unit, range, what)
    };
    let count = Range::nonneg();
    let at_least_1 = Range::at_least(1.0);
    rows.extend([
        t("motor_refused", "fraction", fraction, "Self.motor_delivery below this is a refused motor").search(0.1, 0.9),
        t("escape_samples", "samples", count, "controller samples an escape lasts").search(1.0, 12.0),
        t("escape_creep", "effort", fraction, "forward effort while turning away").search(0.0, 1.0),
        t("flat_clear", "fraction of rays", fraction, "share of a sector's rays clear on flat open ground").grade("N"),
        t("half_ray", "fraction of rays", fraction, "half a ray's share of a sector").grade("N"),
        t("drop_proximity", "1 - d/range", fraction, "floor rows reading this far on average are an edge").search(0.3, 0.8),
        t("wander_hold_min", "samples", count, "lowest hold of a wander's heading").search(2.0, 16.0),
        t("wander_hold_max", "samples", count, "highest hold of a wander's heading").search(8.0, 48.0),
        t("wander_turn_samples_min", "samples", at_least_1, "lowest length of a wander turn").search(1.0, 4.0),
        t("wander_turn_samples_max", "samples", at_least_1, "highest length of a wander turn").search(2.0, 8.0),
        t("wander_turn", "effort", fraction, "turn effort while a wander turn runs").search(0.2, 1.0),
        t("feed_gate", "taste cue", nonneg, "the heuristics feed when the taste cue is above this").search(0.005, 0.5).log(),
        t("blocked_forward", "effort", fraction, "forward effort when the front contact is blocked").search(0.0, 0.5),
        t("chem_response_gate", "chem response", nonneg, "the blind heuristic follows a chem signal at least this strong").search(0.005, 0.2).log(),
        t("chem_trend_gate", "chem trend", nonneg, "a chem trend beyond this either way is rising or falling").search(0.01, 0.3).log(),
        t("blind_turn", "effort", fraction, "turn effort of the blind heuristic's wall and falling-signal turns").search(0.2, 1.0),
        t("browser_sector_gate", "fraction of rays", fraction, "a cone sector sees foliage above this fraction of its rays").search(0.005, 0.2).log(),
        t("browser_gaze_turn", "effort", fraction, "turn effort of the browser's gaze at the richest sector").search(0.2, 1.0),
        t("browser_wander_turn", "effort", fraction, "turn effort of the browser's no-signal wander").search(0.2, 1.0),
        t("browser_wander_flip", "samples", count, "samples the browser's wander holds a side before flipping").search(1.0, 12.0),
        t("strike_proximity", "1 - d/range", fraction, "a body this close is inside the strike capsule").search(0.2, 0.8),
        t("stalk", "effort", fraction, "the ambush's forward effort while stalking a body").search(0.0, 0.3),
        t("near_crown", "1 - d/range", fraction, "a crown this near ahead is a waiting spot").search(0.3, 0.9),
        t("wait_samples", "samples", count, "samples the ambush waits at an empty spot before leaving").search(40.0, 960.0).log(),
        t("leave_samples", "samples", count, "samples the ambush walks off after a fruitless wait").search(4.0, 80.0),
        t("ambush_face_turn", "effort", fraction, "the ambush's turn toward a body it sees off-centre").search(0.1, 1.0),
        t("ambush_leave_turn", "effort", fraction, "the ambush's turn while leaving a spot").search(0.0, 1.0),
        t("ambush_gaze_turn", "effort", fraction, "the ambush's turn toward foliage while seeking a crown").search(0.2, 1.0),
        t("ambush_wander_turn", "effort", fraction, "the ambush's no-signal wander turn").search(0.1, 1.0),
        t("ambush_wander_flip", "samples", count, "samples the ambush's wander holds a side before flipping").search(2.0, 24.0),
    ]);

    // `fauna.founding.*`: the seeder's founders.
    let f = |path: &str, unit: &'static str, range: Range, what: &'static str| {
        r(&format!("fauna.founding.{path}"), unit, range, what)
    };
    rows.extend([
        f("arrival_body", "fraction of adult body", fraction, "a seeded founder arrives at this fraction of its adult body").search(0.3, 1.0).prov("decision"),
        f("arrival_reserve", "fraction of reserve cap", fraction, "a seeded founder arrives with this fraction of its reserve cap").search(0.0, 1.0).prov("decision"),
        f("starter_litter_organic", "organic units", nonneg, "leaf litter laid under each seeded littershredder").search(0.05, 1.0),
        f("starter_litter_mineral_fraction", "fraction of organic", fraction, "mineral share of the starter litter").search(0.005, 0.1),
        f("starter_litter_energy_density", "energy per organic unit", nonneg, "retained energy density of the starter litter").search(0.5, 4.0),
        f("accept_founder_hours", "founder-hours", nonneg, "founder-hours of adult upkeep a component's stock must cover per founder").search(0.1, 6.0),
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
    knob_rules(c)
}

/// The grouped knobs' ordered pairs: a draw's low end at most its high end, a band's
/// lower edge below its upper.
fn knob_rules(c: &FaunaConfig) -> Result<(), String> {
    let (p, l, t) = (&c.predation, &c.life, &c.teachers);
    if p.egg_footfall_min > p.egg_footfall_max {
        return Err(format!(
            "fauna.predation.egg_footfall_min {} is above egg_footfall_max {}",
            p.egg_footfall_min, p.egg_footfall_max
        ));
    }
    if l.twilight_lo > l.twilight_hi {
        return Err(format!(
            "fauna.life.twilight_lo {} is above twilight_hi {}",
            l.twilight_lo, l.twilight_hi
        ));
    }
    if t.wander_hold_min > t.wander_hold_max {
        return Err(format!(
            "fauna.teachers.wander_hold_min {} is above wander_hold_max {}",
            t.wander_hold_min, t.wander_hold_max
        ));
    }
    if t.wander_turn_samples_min > t.wander_turn_samples_max {
        return Err(format!(
            "fauna.teachers.wander_turn_samples_min {} is above wander_turn_samples_max {}",
            t.wander_turn_samples_min, t.wander_turn_samples_max
        ));
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
