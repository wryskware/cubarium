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
                Alias { pattern: "fauna.founders.*.yield_by_food".into(), names: foods.clone() },
                Alias { pattern: "fauna.founders.*.gut.digestion_s".into(), names: foods },
                Alias { pattern: "fauna.founders.*.hunter.prey".into(), names: founders.clone() },
                Alias { pattern: "fauna.founders.*.hunter.prey_weight".into(), names: founders.clone() },
                Alias { pattern: "fauna.founders.*.hunter.prey_juveniles_only".into(), names: founders.clone() },
                Alias { pattern: "fauna.founders.*.hunter.prey_ashore_only".into(), names: founders },
            ],
            Vec::new(),
        )
        .with_ties(ties())
        .with_groups(groups())
    })
}

/// The seven absolute quantities a body's org-unit mass scale carries
/// ([`crate::SpeciesConfig::scale_mass`]), relative to a `SpeciesConfig`. Not
/// `bite_half_stock`: it is in the food's stock units, and the food is not scaled.
pub const MASS_SCALED: [&str; 7] = [
    "body_max",
    "body_min",
    "birth_body",
    "birth_cost",
    "reproduction.surplus_floor",
    "reproduction.egg_organic",
    "growth_max_per_s",
];

/// The animal layer's `derive()` (registry §4): the fields that follow others by ratio
/// when a genome moves the others.
/// - Walking is priced against upkeep (audit §8.1): `motor_respiration_per_s =
///   motor_factor × maintenance_per_s`, the factor today's ratio.
/// - The bite is not tied: the minimal gut's `gut.bite_per_body_s` is its own number, per
///   unit of body, so the mass scale does not carry it either.
/// - The seven absolute quantities follow the lineage's mass scale (audit §1.4); the bite's
///   half-saturation and taste reference, in food units, do not.
/// - The placeholder species is the browser's body: its mass and lifespan follow the
///   browser's.
pub fn ties() -> Vec<Tie> {
    let mut t = vec![
        Tie::new(
            "motor_follows_upkeep",
            "fauna.founders.*.motor_respiration_per_s",
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
        r("maintenance_per_s", "org-units/s per unit body", nonneg, "basal upkeep, paid from reserve then body: daily need f / 7200 s")
            .search(5e-7, 4e-3).log().grade("B"),
        r("bite_half_stock", "org-units at the mouth", nonneg, "K: a bite is want * E / (E + K)")
            .search(1e-4, 5e-2).log().grade("C").prov("tuned"),
        r("n_tissue", "mineral per org-unit", nonneg, "tissue stoichiometry: a bite builds at most mineral / n_tissue")
            .search(0.01, 0.12).log().grade("A"),
        r("body_max", "org-units", nonneg, "adult structure").grade("D"),
        r("growth_max_per_s", "org-units/s", nonneg, "fastest structure grows from the reserve")
            // Includes shipped mass-scaled ceilings and cotrain's x4 maturity box.
            .search(1e-6, 1e-2).log().grade("B").prov("derived"),
        r("body_min", "org-units", Range::positive(), "death threshold, and a newborn's body").evolve().grade("B"),
        r("birth_body", "org-units", nonneg, "structure at which it can pay for a birth").evolve().grade("B"),
        r("birth_cost", "org-units", nonneg, "a gestation's package: the newborn's body_min plus its reserve")
            .evolve().grade("B"),
        r("reserve_cap", "fraction of body", nonneg, "ceiling on new intake into the reserve")
            .search(0.2, 5.0).grade("A"),
        r("drown_depth_m", "m", nonneg, "standing water that kills the heuristic body and a clutch").grade("D"),
        r("energy_density", "energy per org-unit", nonneg, "energy an introduced animal arrives with").grade("N"),
        r("lifespan_median_s", "s", nonneg, "median lifespan (0: none)")
            .search(3600.0, 172800.0).log().grade("B").prov("derived"),
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
        r("reproduction.clutch_max", "eggs", nonneg, "clutch from surplus: most eggs a clutch holds, sized at laying from the reserve above the floor (0: the fixed clutch_size)")
            .search(0.0, 8.0),
        r("reproduction.nest_radius_m", "m", nonneg, "nest-site brake: radius an egg-layer counts its own lineage (bodies and unhatched eggs) in (0 off)")
            .search(0.25, 3.0),
        r("reproduction.nest_crowd_max", "bodies and eggs", nonneg, "nest-site brake: more of its own lineage than this within the radius refuses a clutch")
            .search(1.0, 30.0),
        r("reproduction.nest_retry_s", "s", nonneg, "a refused layer (nest site, body ceiling) waits this long before trying again")
            .search(5.0, 600.0).log(),
        r("reproduction.nest_seek", "switch", Range::fraction(), "SHREDDER-NEST: refused by a crowded nest, it walks to the nearest free egg face within nest_seek_m and lays there (false: it waits out its retry where it is)").optional(),
        r("reproduction.nest_seek_m", "m", nonneg, "SHREDDER-NEST: how far a refused layer looks for an uncrowded egg face (level distance, within one step of its height)")
            .search(0.5, 8.0),
        r("reproduction.nest_seek_give_up_s", "s", nonneg, "SHREDDER-NEST: a walk off a crowded nest not ended by a clutch is given up after this; the body then waits out its retry")
            .search(10.0, 600.0).log(),
        r("reproduction.clutch_hold_max_s", "s", nonneg, "clutch from surplus: longest a ready layer waits for a full clutch_max before laying what it can (0 lays at once)")
            .search(0.0, 7200.0),
        r("reproduction.egg_fungus_radius_m", "m", nonneg, "fungal egg site: an egg goes within this of a fungus holding cap tissue (0: its own face)")
            .search(0.0, 2.0),
        r("reproduction.brood_share", "fraction of net income", Range::fraction(), "the brood (1 - kappa): share of a mature egg-layer's net income earmarked for its next full clutch; growth draws only above it, hunger ignores it (0 off)")
            .search(0.0, 1.0),
        r("reproduction.litters_per_year", "conceptions per year", nonneg, "breeding season: at most this many conceptions (gestations opened or clutches laid) per Cubarium year, a pack's counted per pack (0: no season)"),
        r("reproduction.breeding_phase", "year phase", Range::fraction(), "breeding season: the year phase its window opens at (0: a dry season's start)"),
        r("reproduction.breeding_window", "share of the year", nonneg, "breeding season: the window's width (1 or more: the whole year)"),
        r("reproduction.gestation_upkeep_factor", "x basal upkeep", nonneg, "pregnancy: a gestating body's basal upkeep is multiplied by this (1: no raise)"),
    ]
}

/// [`crate::FounderPhysiology`]'s own rows, relative to `fauna.founders.<lineage>`.
fn founder_rows() -> Vec<Row> {
    let nonneg = Range::nonneg();
    let fraction = Range::fraction();
    vec![
        r("fruit_bite_half_stock", "org-units at the mouth", nonneg, "fruit-only K override; absent uses core.bite_half_stock, other foods retain core K").optional().prov("derived"),
        r("give_up_fraction", "fraction of the fill rate", nonneg, "lineage override of teachers.give_up_fraction; absent inherits, zero disables giving up").optional().prov("derived"),
        r("dispersal.group_max", "bodies", nonneg, "closed-world dispersal: a transitioning young leaves when its pack or herd's established members plus itself exceed this; dependent young never count"),
        r("dispersal.enabled", "switch", fraction, "closed-world dispersal switch (false by default; true enables it)").optional(),
        r("dispersal.at", "transition", nonneg, "closed-world dispersal transition: adult (breeding size) or independent (pack pup age)").optional(),
        r("motor_respiration_per_s", "org-units/s per unit body at full cruise", nonneg, "paid motion")
            .search(2e-5, 1e-2).log().grade("B"),
        r("step_up_m", "m", nonneg, "tallest riser stepped (rounded down to voxels)")
            .search(0.1, 1.5).digest().grade("B").prov("measured"),
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
        // The amphibious table (`Amphibious`, AMPHIBIOUS-SNAIL): the ripple-snail's only,
        // optional on the others. `on_land` is a switch: no row.
        r("amphibious.dry_ground_s", "s", Range::positive(), "out of water, its drying clock fills in this on bone-dry ground (pore fill 0), by night in dry weather").optional().log().prov("decision"),
        r("amphibious.wet_ground_s", "s", Range::positive(), "... and in this on saturated ground (pore fill 1, or a film of water)").optional().log().prov("decision"),
        r("amphibious.rehydrate_per_s", "clock fraction/s", nonneg, "the clock falls this fast in water (standing water within water_margin_m)").optional().prov("decision"),
        r("amphibious.step_moisture_min", "pore fill 0..1", fraction, "the least ground moisture a step onto a face out of water needs").optional().prov("decision"),
        r("amphibious.return_fraction", "clock fraction", nonneg, "at night or in rain its heuristic heads for water past this, and it is held to the water-bound rule until it drinks").optional().prov("decision"),
        r("amphibious.day_return_fraction", "clock fraction", nonneg, "... and past this by day in dry weather (the activity bias)").optional().prov("decision"),
        r("amphibious.day_drying_factor", "x", nonneg, "the clock runs this many times as fast while the sun is up").optional().prov("decision"),
        r("amphibious.rain_drying_factor", "x", nonneg, "... and this many times as fast while rain falls (0: no drying in rain)").optional().prov("decision"),
        r("amphibious.lethal_fraction", "clock fraction", Range::positive(), "it dies of desiccation at this (from 1 up it cannot feed)").optional().prov("decision"),
        r("amphibious.water_search_m", "m", nonneg, "how far its water reflex looks for standing water (a world query)").optional(),
        r("amphibious.retract_hold_s", "s", nonneg, "a threat seen or felt makes it stop and close for this (0: it flees instead)").optional().prov("decision"),
        r("amphibious.retract_armour", "fraction", fraction, "a hit on it while retracted is reduced by this, on top of its armour").optional().prov("decision"),
        r("amphibious.roam_search_m", "m", nonneg, "at night and in or after rain, not thirsty and not feeding, it walks to the land face with the most film within this, or out of water to the nearest wet land face (0: no roam)").optional(),
        r("amphibious.roam_after_rain_s", "s", nonneg, "the roam hours last this long after rain stops").optional(),
        // SNAIL-LAND: eggs on land (EggSite::Shore) and their risks.
        r("amphibious.egg_water_max_m", "m", nonneg, "shore eggs: the egg face is within this horizontal distance of standing water").optional(),
        r("amphibious.egg_climb_max_m", "m", nonneg, "shore eggs: the egg face stands at most this far above that water's surface").optional(),
        r("amphibious.egg_moisture_min", "pore fill 0..1", fraction, "shore eggs: the least ground moisture of the egg face").optional(),
        r("amphibious.egg_drown_s", "s", nonneg, "shore eggs: an egg under standing water this long without a break dies to the litter").optional().prov("decision"),
        r("amphibious.egg_dry_moisture", "pore fill 0..1", fraction, "shore eggs: an egg on ground drier than this ...").optional().prov("decision"),
        r("amphibious.egg_dry_s", "s", nonneg, "... for this long without a break dies to the litter").optional().prov("decision"),
        // SNAIL-LAND-2: the trip's reach and the giving-up density.
        r("amphibious.egg_search_m", "m", nonneg, "a ready layer looks this far for a shore egg face it can afford whose nest is not crowded").optional(),
        r("amphibious.graze_give_up", "fraction of a site's film capacity", nonneg, "wanting food on a face with less film than this, it walks to a richer face (0: it stays on any film it tastes)").optional(),
        r("amphibious.graze_worth", "fraction of a site's film capacity", nonneg, "... one holding at least this per snail on it (itself counted) ...").optional(),
        r("amphibious.graze_search_m", "m", nonneg, "... within this distance (in water at any hour, out of water in its roam hours)").optional(),
        r("contact_height_fraction", "x body height", nonneg, "the contact receptors' height").digest(),
        r("mouth_reach_length_fraction", "x body length", nonneg, "the mouth's reach ahead of the footprint")
            .evolve().digest(),
        r("yield_by_food.*", "fraction", fraction, "assimilation yield of one food class").search(0.05, 0.9),
        r("organ_structure_fraction", "fraction of body", Range::closed(0.0, 1.0).open_hi(), "sensor/organ share of structure")
            .grade("N"),
        r("flower_ambush_m", "m", nonneg, "MIMIC: holds an ambush this close to a flower"),
        // `opportunist` (CATCH-SWITCH: the hunter table is the catch's, on with `catch_bellwings`) is a switch: no row.
        // `pollen_by_contact` (SEEDPORTER-POLLEN: its body pollinates the vine flowers it touches) is a switch: no row.
        r("catch_ceiling_m", "m above hunter base", nonneg, "MIMIC: highest bellwing base a catch can reach"),
        r("catch_gain_min", "organic units", nonneg, "MIMIC: minimum yield less expected strike upkeep cost"),
        r("catch_foray_m", "m", nonneg, "MIMIC-2: sensed low bellwing cluster search radius; 0 disables forays"),
        r("catch_foray_min", "bodies", nonneg, "MIMIC-2: minimum sensed profitable visitors before a fruit-search detour"),
        r("catch_foray_s", "s", nonneg, "MIMIC-2: maximum catch foray without a reward, before avoiding its patch"),
        r("active_window.before_rise_h", "h on the 24-h day clock", nonneg, "it wakes this long before sunrise on top of its rhythm (LURE: dawn flight)").prov("decision"),
        r("active_window.after_set_h", "h on the 24-h day clock", nonneg, "it stays awake this long after sunset on top of its rhythm (LURE: dusk flight)").prov("decision"),
        // RHYTHMS-CADENCE (`crate::Rhythm`, read while `active` is "Leaning"); `rhythm.shade_is_night` is a switch: no row.
        r("rhythm.day", "chance", fraction, "a leaning's waking chance with the sun up, beyond twilight_h of the horizon"),
        r("rhythm.night", "chance", fraction, "a leaning's waking chance with the sun down, beyond twilight_h of the horizon"),
        r("rhythm.twilight", "chance", fraction, "a leaning's waking chance at sunrise and sunset (above both: a dawn and dusk peak)"),
        r("rhythm.twilight_h", "h on the 24-h day clock", nonneg, "the chance eases from the twilight value to the day or night value over this either side of the horizon"),
        r("rhythm.rest_mean_h", "h on the 24-h day clock", nonneg, "a rest bout's mean length (rests begin at random so the awake share follows the chance; 0: never rests)"),
        r("rhythm.rest_min_h", "h on the 24-h day clock", nonneg, "a rest bout's shortest length (the rest is this plus an exponential draw)"),
        // RHYTHMS-CADENCE stop and go (`crate::Cadence`).
        r("cadence.cycle_s", "s", nonneg, "stop and go: one moving bout and its pause, a pause at a random point in each (0: steady cruise)"),
        r("cadence.pause_min_s", "s", nonneg, "stop and go: a pause's shortest (uniform draw)"),
        r("cadence.pause_max_s", "s", nonneg, "stop and go: a pause's longest"),
        r("cadence.speed_min", "x asked forward", nonneg, "stop and go: a bout's slowest pace (uniform draw)"),
        r("cadence.speed_max", "x asked forward", nonneg, "stop and go: a bout's fastest pace"),
        r("sleep_upkeep", "x basal upkeep", fraction, "upkeep while asleep (and a flyer perched out the rain)").search(0.1, 1.0).prov("decision"),
        r("wake_hunger", "hunger 0..1", fraction, "the emergency latch goes on (awake in any phase) at this reserve hunger").search(0.1, 1.0),
        r("emergency_clear_hunger", "hunger 0..1", fraction, "the emergency latch goes off below this reserve hunger")
            .search(0.0, 0.9).prov("decision"),
        // The minimal gut (`Gut`, design/metabolism-meals-2026-10-02.md repair 2).
        r("gut.stomach_k", "raw org-units per unit current body", nonneg, "k: stomach capacity K = k x body")
            .search(0.002, 1.0).log().prov("decision"),
        r("gut.bite_per_body_s", "raw org-units/s per unit current body", nonneg, "b: a bite's rate at full effort (default 2k / tau)")
            .search(1e-5, 1e-2).log().prov("derived"),
        r("gut.digestion_s.*", "s", Range::positive(), "tau: a food class's pool empties at S / tau into the reserve's room")
            .search(5.0, 2000.0).log().prov("decision"),
        r("gut.s0", "fraction of K", fraction, "appetite floor: a fed body wants food at S <= s0 K")
            .search(0.0, 0.8).prov("decision"),
        r("gut.full_tolerance", "fraction of K", Range::closed(0.0, 1.0).open_lo(), "the eating latch goes off at S >= this x K")
            .prov("decision"),
        r("gut.refill_gap", "fraction of K", fraction, "s_start is capped at full_tolerance - this, so appetite never overlaps the full event")
            .search(0.0, 0.5).prov("decision"),
        r("gut.mass_exponent", "exponent", Range::closed(0.0, 1.0).open_lo(), "e: intake at a full stomach goes as body^e (tau x (body/body_max)^(1-e), bite x (body/body_max)^(e-1)); 1 is the adult's per-mass gut at every size")
            .search(0.5, 1.0),
        r("armour", "fraction", fraction, "a hit is reduced by 1 - armour"),
        r("adult_size_window_factor", "x own body", Range::at_least(1.0), "LANTERNJAW-LIFE: a body at or past birth_body counts as this x its body in every hunter's size window (formidable when grown; 1 its own size)")
            .search(1.0, 30.0).log(),
        r("burr_coat", "factor", fraction, "how readily a grass burr holds to this coat: multiplies the plant's per-contact pickup chance"),
        // `senses.live` and `senses.scent` are switches: no rows.
        r("senses.sight_m", "m", nonneg, "sight by day: a moving body within this (3D) inside its field, terrain not blocking the line of sight (0: blind)"),
        r("senses.sight_night_m", "m", nonneg, "sight's range below the day line"),
        r("senses.sight_still_fraction", "fraction", nonneg, "a still body is seen within this fraction of the range"),
        r("senses.sight_half_angle_deg", "deg", nonneg, "sight's field half-angle from the heading (180: all around)"),
        r("senses.hearing_m", "m", nonneg, "hearing's range, all around (0: deaf)"),
        r("senses.hearing_min", "m/s at the default exponents", nonneg, "the faintest received loudness heard: emission speed^a x length^b over the distance, through terrain x occlusion"),
        r("senses.tremor_m", "m", nonneg, "tremor's range for bodies on the ground (0: none)"),
        r("senses.tremor_min", "organic x m/s / m^2", nonneg, "the faintest footfall felt (mass x speed / d^2, through terrain x occlusion)"),
        r("senses.smell_min", "scent units", nonneg, "the faintest scent level smelled on the scent field (0: no smell)").log(),
        // `senses.nir` is a switch: no row.
        r("senses.light_m", "m", nonneg, "the photoreceptor's range for a beacon (3D; 0: no light sense)"),
        r("senses.light_min", "contrast", nonneg, "the faintest beacon level read: contrast x 1 / (1 + (d / half_m)^2)").log(),
        r("senses.cyan_m", "m", nonneg, "FLOWER-FRUIT: a still cyan light (a glowcap) draws only within this (3D); 0: light_m alone"),
        r("senses.loom_m", "m", nonneg, "a moving body of another lineage within this (3D) looms: a shadow falling on it (0: none)"),
        r("senses.feeding_sight_fraction", "fraction", nonneg, "S2: head down cropping between scans, sight reaches this fraction of its range (1: no change)"),
        r("senses.kin_tracks", "bodies", nonneg, "S2: at most this many of its own lineage weighed per remote sensing, nearest first"),
        r("senses.still_gain", "factor", nonneg, "RHYTHMS-CADENCE: still for still_settle_s, its sight, hearing and tremor reach this x farther and hear and feel this x fainter (1: no change)"),
        r("senses.still_settle_s", "s", nonneg, "RHYTHMS-CADENCE: still this long before still_gain applies"),
        // S2's ground response (`Vigilance`); `vigilance.mode` is a switch: no row.
        r("vigilance.flee_start_m", "m", nonneg, "S2: flight starts with threats (seen moving or near, heard, felt) within this"),
        r("vigilance.flee_stop_m", "m", nonneg, "S2: flight stops beyond this of the last-known threat..."),
        r("vigilance.calm_s", "s", nonneg, "S2: ...once no threat has come within the stop distance for this long"),
        r("vigilance.group_min", "threats", nonneg, "S2: an adult flees only from at least this many threats (a juvenile from one)"),
        r("vigilance.run_noise", "x emission", nonneg, "S2: its emission while it flees or bolts (a run is loud)"),
        r("vigilance.sleep_hearing", "fraction", fraction, "RHYTHMS-CADENCE: asleep, it hears a running body at this share of its awake sensitivity and wakes (0: sleeps through)"),
        r("vigilance.scan_every_s", "s", nonneg, "S2: cropping between scan pauses (0: never scans)"),
        r("vigilance.scan_s", "s", nonneg, "S2: a scan pause"),
        r("vigilance.scan_kin_m", "m", nonneg, "HERD-PULL many eyes: kin it knows within this share the watch (0: it scans alone)"),
        r("vigilance.scan_kin_gain", "x scan_every_s", nonneg, "HERD-PULL many eyes: each such kin lengthens the cropping between scans by this x scan_every_s"),
        r("vigilance.scan_kin_max", "kin", nonneg, "HERD-PULL many eyes: ...counting at most this many (a big herd still scans)"),
        r("vigilance.orient_s", "s", nonneg, "S2: it faces a newly sensed approaching threat this long (0: never orients)"),
        r("vigilance.alarm_emission_factor", "x walking emission", nonneg, "S2: a kin heard this loud (x its walking emission) is bolting, and it bolts too (0: no alarm)"),
        r("vigilance.alarm_bolt_s", "s", nonneg, "S2: an alarm bolt lasts this long"),
        r("vigilance.alarm_refractory_s", "s", nonneg, "S2: after a bolt it hears no alarm this long"),
        r("vigilance.freeze_tremor", "organic x m/s / m^2", nonneg, "S2: footfall (left + right) that freezes it (0: never)"),
        r("vigilance.withdraw_tremor", "organic x m/s / m^2", nonneg, "S2: footfall at which it also turns away in place"),
        r("vigilance.freeze_s", "s", nonneg, "S2: a freeze lasts this long after the last tremor that set it"),
        r("vigilance.refuge_search_m", "m", nonneg, "S2: an arboreal body in flight runs to a stand it climbs within this"),
        // FEAR (`vigilance.watch_calm` is a switch: no row).
        r("vigilance.calm_margin_m", "m", nonneg, "FEAR: it walks off a hunter it watches (asleep, or resting and not facing it) closer than this (3D; 0 never)"),
        r("vigilance.calm_facing_deg", "deg", nonneg, "FEAR: a still hunter heading within this of the bearing to it faces it (a threat)"),
        r("vigilance.lost_calm_s", "s", nonneg, "FEAR-2: a flight or refuge watch whose hunter it has not known within the stop distance for this long ends (0 never)"),
        // HERD-PULL (`Herding`); `herding.young_only` is a switch: no row.
        r("herding.comfort_m", "m", nonneg, "HERD-PULL: a heuristic body whose nearest known kin is beyond this drifts back toward the kin it knows (0: off)"),
        r("herding.pull", "fraction", fraction, "HERD-PULL: the pull's share of its walking turn (1: straight at the kin's mean point)"),
        r("herding.herd_max", "kin", nonneg, "HERD-PULL: with more kin known than this the pull is scaled by this over their number (0: never fades)"),
        r("herding.memory_s", "s", nonneg, "HERD-PULL: with no kin in sense, the kin it last sensed draws it this long (0: only kin in sense)"),
        // HUNKER (`Hunker`); `hunker.enabled` is a switch: no row.
        r("hunker.run_start_m", "m", nonneg, "HERD: run from a pressing hunter within this (herd_enabled)"),
        r("hunker.catch_m", "m", nonneg, "HERD: a moving faster hunter inside this catches it (0 off)"),
        r("hunker.escape_probe_m", "m", nonneg, "HERD: motor walk distance checking a blocked escape (0 off)"),
        r("hunker.shelter_seek_m", "m", nonneg, "HERD: seek a known closed adult within this"),
        r("hunker.shelter_margin_m", "m", nonneg, "HERD: gap beyond summed footprints and base height tolerance for shelter"),
        r("hunker.shelter_exposure", "fraction", fraction, "HERD: fraction of strikes that reach a juvenile behind an adult shield"),
        r("hunker.enter_m", "m", nonneg, "HUNKER: it clamps down under its shield at an approaching threat within this"),
        r("hunker.leave_m", "m", nonneg, "HUNKER: ...and comes out once none has been within this for the calm"),
        r("hunker.calm_s", "s", nonneg, "HUNKER: the calm at full condition"),
        r("hunker.hunger_calm_exp", "exponent", nonneg, "HUNKER: calm x condition^this (a hungrier body comes out sooner)"),
        r("hunker.min_condition", "fraction", fraction, "HUNKER: below this condition it does not hunker"),
        r("hunker.shield", "fraction", fraction, "HUNKER: chance a strike glances off a grown, unwounded hunkered body"),
        r("hunker.shield_growth_exp", "exponent", nonneg, "HUNKER: the shield grows as (body / adult body)^this"),
        r("hunker.wound_opens", "x wound", nonneg, "HUNKER: a wound w takes this x w of the shield away"),
        r("hunker.pry_per_member_s", "1/s", nonneg, "HUNKER: one hunter in contact pries it open at this (1 opens)"),
        r("hunker.resist_per_s", "1/s", nonneg, "HUNKER: it resists the prying at this (above one member: one cannot open it)"),
        r("hunker.pry_reach_m", "m", nonneg, "HUNKER: a hunter is in contact within half their summed lengths plus this"),
        r("hunker.opened_s", "s", nonneg, "HUNKER: turned over, it is open (no shield, no armour) this long"),
        r("hunker.ring_m", "m", nonneg, "HUNKER: kin within this is beside it (bunched; hunkered, a ring)"),
        r("hunker.ring_resist_per_kin", "x resist", nonneg, "HUNKER: each hunkered kin within ring_m adds this x the resistance"),
        r("hunker.ring_kin_max", "kin", nonneg, "HUNKER: ...counting at most this many"),
        // GAIT's sprint (`Sprint`).
        r("sprint.body_lengths_per_s", "BL/s", nonneg, "GAIT: top speed a rush or ground flight asks for (at or below the cruise: no sprint)"),
        r("sprint.endurance_s", "s", nonneg, "GAIT: seconds at top speed from rested to spent (0: no sprint)"),
        r("sprint.recovery_s", "s", nonneg, "GAIT: seconds off the sprint from spent to rested"),
        r("sprint.resume_fraction", "fraction", fraction, "GAIT: a winded body sprints again once rested to this"),
        // DROP's jump down when trapped (`DropRule`).
        r("drop.after_s", "s", nonneg, "DROP: trapped this long (off its route graph, no walking way back on, no glide) it jumps down"),
        r("drop.max_m", "m", nonneg, "DROP: deepest jump down an adult takes (a smaller body: x its height / the adult's; at or under the step: no jump)"),
        r("drop.wound_per_m", "wound per m", nonneg, "DROP: wound a landing adds per metre dropped beyond step_down_m (1 kills)"),
        // WADE-OUT (`WadeRule`).
        r("wade.swim_out", "switch", fraction, "WADE-OUT: held in water past its depth (or in a pocket ringed by it), it swims the least-deep way out within its drown clock (false: it stays)").optional(),
        r("agility", "fraction", fraction, "the dodge's species factor"),
        r("mass_scale", "x placeholder mass", Range::positive(), "the org-unit mass scale the body was built at")
            .digest().prov("decision"),
        r("taste_reference_food", "organic units per site", Range::positive(), "default food reference for taste saturation; independent of body mass and scent emission")
            .digest().prov("decision"),
        r("taste_reference", "organic units per site", Range::positive(), "food-reference override for taste saturation (absent: a film-only diet derives film_capacity_per_m2 x cell area on the world; else taste_reference_food)")
            .optional().digest().prov("decision"),
        // The carnivore's table (`Hunter`): the lanternjaw's only, optional on the others.
        r("hunter.strike_power", "wound per hit", nonneg, "a hit adds power x (S_pred / S_prey) x (1 - armour)").optional(),
        r("hunter.prey_max_ratio", "x own body", nonneg, "the size window: prey at most this x its own").optional(),
        r("hunter.strike_reach_length_fraction", "x body length", nonneg, "the strike capsule's reach").optional(),
        r("hunter.vertical_reach_m", "m", nonneg, "rearing and jumping reach above the standing surface; 0 uses only the jaw band").optional(),
        r("hunter.jaw_band_height_fraction", "x body height", nonneg, "the jaw band's top").optional(),
        r("hunter.jaw_forward_length_fraction", "x body length", nonneg, "the jaw's point ahead of the body's centre").search(0.2, 0.8).optional(),
        r("hunter.windup_s", "s", nonneg, "the crouch").optional(),
        r("hunter.strike_warning_scale", "x looming range", fraction, "SP-FLUSH: visual warning range of this hunter's strike (1: whole-body lunge; smaller: short arm grab); closing-body looming is unchanged").optional(),
        r("hunter.lunge_s", "s", nonneg, "the lunge").optional(),
        r("hunter.recovery_s", "s", nonneg, "the recovery, at half speed and unable to strike").optional(),
        r("hunter.strike_cost_s", "s of awake upkeep", nonneg, "a strike's cost, hit or miss").optional(),
        r("hunter.lure_power", "x awake upkeep", nonneg, "a lit lure's cost").optional(),
        r("hunter.prey_weight.*", "fraction", fraction, "LANTERNJAW-DIET: its taste for one prey lineage, a factor on the attack chance (1: none beyond profitability and the search image)").optional(),
        // The flyer's table (`Flight`, F1): the bellwing's only, optional on the others.
        r("flight.span_m", "m", Range::positive(), "wingspan: the collision sphere's diameter").optional().prov("decision"),
        r("flight.band_max_m", "m", nonneg, "the ceiling: the base's most height over the support face below").optional().prov("decision"),
        r("flight.flight_factor", "x basal upkeep", nonneg, "a second airborne at cruise").optional().prov("decision"),
        r("flight.hover_factor", "x basal upkeep", nonneg, "a second hovering").optional().prov("decision"),
        r("flight.climb_work_per_m", "org-units per m per unit body", nonneg, "lift work going up; descent is free").optional(),
        r("flight.takeoff_s", "s of flight", nonneg, "a takeoff's cost").optional().prov("decision"),
        r("flight.foliage_speed_factor", "x speed cap", fraction, "the speed cap inside a crown's foliage layers").optional().prov("decision"),
        r("flight.shower_rain_reference_m_per_s", "m/s of rain", Range::positive(), "R0: flight costs x (1 + rain rate / R0) in a shower").optional(),
        r("flight.lift_speed_fraction", "x cruise speed", nonneg, "vertical speed at full lift, either way").optional(),
        r("flight.egg_headroom_m", "m", nonneg, "a flyer lays only on a face with this clear over its hatchling's body").optional(),
        r("flight.egg_air_refresh_s", "s", nonneg, "the open air a flyer's egg site must reach is rebuilt at most this often").optional(),
        r("flight.rain_ground_m_per_s", "m/s of rain", nonneg, "at or over this rain rate a flyer comes down (under cover when it can) and neither takes off nor sips").optional(),
        r("flight.rain_ground_hysteresis_m_per_s", "m/s of rain", nonneg, "a rain-grounded flyer flies again once the rain is under the threshold less this").optional(),
        r("flight.nest_search_m", "m", nonneg, "VINE-C: a vine nester ready to lay with no vine face in reach flies to the nearest covered wall face within this that would take its clutch").optional(),
        r("flight.flush_reaction_s", "s", nonneg, "FLUSH: a hunter sensed looming (its strike begun, or closing) within loom_m sends it off this long after").optional(),
        r("flight.flush_speed_factor", "x cruise speed", nonneg, "FLUSH: its escape burst's speed, off the threat's line (0: no flush)").optional(),
        r("flight.flush_climb_fraction", "x burst speed", nonneg, "FLUSH: the burst's climb").optional(),
        r("flight.flush_burst_s", "s", nonneg, "FLUSH: the burst's length").optional(),
        r("flight.flush_s", "s", nonneg, "FLUSH: after the takeoff it flies on away and up, neither sipping nor landing by choice, this long").optional(),
        // The glider's table (`Glider`, F3): the seedporter's only, optional on the others.
        r("glider.climb_min_stand_m", "m", nonneg, "the shortest stand whose stem it climbs (crown top over the support)").optional().prov("decision"),
        r("glider.glide_ratio", "m across per m down", Range::positive(), "a glide's horizontal metres per metre of descent").optional().prov("decision"),
        r("glider.glide_speed_fraction", "x ground cruise", Range::positive(), "a glide's speed, a multiple of the lineage's cruise").optional().prov("decision"),
        r("glider.glide_cost_factor", "x basal upkeep", nonneg, "a second gliding, upkeep included").optional().prov("decision"),
        r("glider.glide_min_height_m", "m", nonneg, "a glide starts only from this height over the landing surface").optional().prov("decision"),
        r("glider.roost_search_m", "m", nonneg, "at its rest hour or in a downpour a grounded glider walks to the nearest climbable stand within this").optional(),
        r("glider.fruit_search_m", "m", nonneg, "a hungry grounded glider on its heuristic walks to the nearest fruiting stand within this (a world query, not a sense)").optional(),
        r("glider.roost_budget_m", "m", nonneg, "at its rest hour a glider on its heuristic roosts in a vaulttree crown, else on a vine-climbed ledge top, within this (0: off)").optional(),
        r("glider.roost_reserve_share", "fraction of reserve above emergency floor", fraction, "a preferred roost's trip (walk and climb) may spend this share").optional(),
        r("glider.roost_ledge_min_m", "m", nonneg, "a roost ledge stands at least this over the foot of its vine wall").optional(),
        // Gut seeds (`GutSeeds`, F3): the seedporter's only, optional on the others.
        r("gut_seeds.seeds_per_fruit_organic", "org-units per org-unit of fruit", fraction, "share of a fruit meal that is the source's seeds, carried in the gut").optional(),
        r("gut_seeds.gut_transit_s", "s", nonneg, "a meal's seeds are voided at the body's face this long after it").optional(),
        // The pack table (`Pack`, roster R3): the chorister's only, optional on the others.
        r("pack.pack_radius_m", "m", nonneg, "the size window sums the bodies of packmates within this").optional().prov("decision"),
        r("pack.pack_size", "adults", nonneg, "adults per pack at founding (the first is the breeder)").optional().prov("decision"),
        r("pack.den_cover_m", "m", nonneg, "a den has solid within this over its face").optional().prov("decision"),
        r("pack.den_radius_m", "m", nonneg, "nursery/provisioning radius; pups stay here, resting adults use rest_radius_m").optional().prov("decision"),
        r("pack.litter_size_max", "young", nonneg, "a litter is 2 up to this").optional().prov("decision"),
        r("pack.litter_interval_s", "s", nonneg, "the breeder's litters are at least this far apart").optional().prov("decision"),
        r("pack.independence_age_s", "s", nonneg, "a pup cannot strike and stays at the den until this age").optional().prov("decision"),
        r("pack.provision_reserve_fraction", "fraction of reserve cap", fraction, "an adult at the den provisions a pup while its reserve is above this").optional().prov("decision"),
        r("pack.provision_rate_per_s", "org-units/s per unit giver body", nonneg, "the provisioning transfer").optional(),
        r("pack.provision_efficiency", "fraction", fraction, "share of a transfer the pup receives; the rest is respired").optional().prov("decision"),
        r("pack.territory_radius_m", "m", nonneg, "a pack never paths within this of another pack's den").optional().prov("decision"),
        r("pack.flank_offset_m", "m", nonneg, "flankers go this far either side of the target").optional(),
        r("pack.chase_give_up_s", "s", nonneg, "a chase is dropped after this long without progress (closing or working the target)").optional().prov("decision"),
        r("pack.chase_progress_m", "m", nonneg, "CHURN: closing this much nearer than the pack's nearest so far restarts the give-up clock").optional(),
        r("pack.switch_margin", "ratio", nonneg, "CHURN: in a stalled chase, another target takes the hunt only when worth this many times the current one (prospect x body)").optional(),
        r("pack.switch_stall_s", "s", nonneg, "CHURN2: a chase is open to a switch only after this long without progress (below chase_give_up_s)").optional(),
        r("pack.close_in_m", "m", nonneg, "STRIKE: within this of its target, on its level, a member leaves its route and steers to its stand-off facing the prey (0: off)").optional(),
        r("pack.stand_off_reach_fraction", "fraction of the strike's reach", fraction, "STRIKE: the stand-off is the jaw's reach plus this share of the strike's reach from the prey").optional(),
        r("pack.pit_search_m", "m", nonneg, "PIT: a member off its route graph walks back onto it by its own body's shortest way, searched this far (0: off)").optional(),
        r("pack.route_swing_voxels", "voxels per controller period", nonneg, "PIT-2: turning onto its route's next leg, a member walks no faster than carries it this far across its line in a period (0: no limit)").optional(),
        r("pack.herd_shun_m", "m", nonneg, "STRIKE-2: a pack dropping an armoured target alive also shuns its armoured kin within this (0: the target alone)").optional(),
        r("pack.route_reach_fraction", "fraction", fraction, "prey is taken only within this share of cruise x chase_give_up_s by route").optional(),
        r("pack.den_flood_wade_fraction", "fraction of an adult's wade depth", nonneg, "DEN-FLOOD: a den counts as flooded when its standing water is deeper than this share of an adult's wade depth").optional(),
        r("pack.route_refresh_s", "s", nonneg, "a pack's walking route components (only prey in its own is taken) are rebuilt at most this often").optional(),
        r("pack.chase_reserve_floor", "fraction of reserve cap", fraction, "a chase is dropped when a member's reserve falls under this").optional(),
        r("pack.call_s", "s", nonneg, "a member calls this long after the target changes or it strikes").optional().prov("decision"),
        r("pack.den_search_m", "m", nonneg, "den search radius around founding/current range; dry reachable roofs, darkest first").optional().prov("decision"),
        r("pack.follow_radius_m", "m", nonneg, "an idle adult walks back to within this of the breeder").optional(),
        r("pack.rest_radius_m", "m", nonneg, "an adult rests or sleeps this close to the den centre, on its level (capped by den_radius_m)").optional(),
        r("pack.home_reserve_share", "fraction of reserve above emergency floor", fraction, "the route home may spend this share on awake upkeep and motor cost").optional(),
        r("pack.stalk_margin", "fraction of the prey's hearing floor", nonneg, "S2: beyond commit_m a member closes no louder than this share of its prey's hearing floor (0: no stalk)").optional(),
        r("pack.breed_reserve_fraction", "fraction of reserve cap", fraction, "the breeder conceives only with its reserve at this fraction of its cap or more (0: the surplus rule alone)").optional(),
        r("pack.regurgitate_reserve_fraction", "fraction of reserve cap", fraction, "PUP: an adult at the den regurgitates its meal to a pup only with its own reserve at this fraction of its cap or more; below it it keeps the meal (0: its condition never matters)").optional(),
        r("pack.commit_m", "m", nonneg, "S2: ...and rushes within this of its prey").optional(),
        r("pack.select_kin_m", "m", nonneg, "PACK-TUNE: kin of a target's lineage within this lower its prospect").optional(),
        r("pack.select_kin_factor", "factor", fraction, "PACK-TUNE: each such kin multiplies the prospect by this").optional(),
        r("pack.select_kin_max", "kin", nonneg, "PACK-TUNE: ...counting at most this many").optional(),
        r("pack.select_aware_factor", "factor", fraction, "PACK-TUNE: a target aware of a member: prospect x this").optional(),
        r("pack.select_wound_gain", "x wound", nonneg, "PACK-TUNE: a wounded target: prospect x (1 + this x wound)").optional(),
        r("pack.select_airborne_factor", "factor", fraction, "PACK-TUNE: a target in flight (in reach): prospect x this").optional(),
        r("pack.select_body_exp", "exponent", nonneg, "PACK-TUNE: score = prospect x body^this / (1 + distance / select_judge_m)").optional(),
        r("pack.select_judge_m", "m", nonneg, "PACK-TUNE: ...the distance that halves the score").optional(),
        r("pack.select_straggler_m", "m", nonneg, "HERD-PULL: score x (1 + select_straggler_gain x min(d, this) / this), d the prey's nearest awake kin (0: off)").optional(),
        r("pack.select_straggler_gain", "factor", nonneg, "HERD-PULL: ...the gain for a prey with no kin within select_straggler_m (0: off)").optional(),
        r("pack.select_min_prospect", "fraction", fraction, "PACK-TUNE: a pack commits only to a prospect of at least this").optional(),
        r("pack.break_off_below", "fraction", fraction, "PACK-TUNE: in a chase, a target whose prospect falls under this is broken off").optional(),
        r("pack.shun_s", "s", nonneg, "PACK-TUNE: a target dropped alive is not taken again for this").optional(),
        // The counter-strike (`Counter`, HUNT-ESCAPE): the lanternjaw's only, `None` on the others.
        r("counter.reach", "m", nonneg, "HUNT-ESCAPE: a grown body answers a strike whose jaw ends within this of its footprint").optional(),
        r("counter.damage", "wound at equal size", nonneg, "HUNT-ESCAPE: the answer wounds the attacker damage x (S_self / S_attacker) x (1 - its armour)").optional(),
        r("counter.cost", "s of awake upkeep", nonneg, "HUNT-ESCAPE: the answer's cost to the answering body").optional(),
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
        r("fauna.parasite_take_body_per_day", "rider's own body per ecological day", Range::nonneg(), "LANTERNJAW-LIFE: a rider's take from its host, into its reserve; grows with the rider")
            .search(0.5, 8.0).log().grade("N"),
        r("fauna.parasite_host_bear_per_day", "host body per ecological day", Range::nonneg(), "LANTERNJAW-LIFE: the most a host gives a rider; a rider wanting more has outgrown it and drops off, a host that cannot bear a hatchling is no host")
            .search(0.1, 2.0).log().grade("N"),
        r("fauna.parasite_detach_body_fraction", "fraction of lineage body_max", Range::closed(0.0, 1.0).open_lo(), "a rider drops off at this size")
            .search(0.15, 0.5).grade("N"),
        // LJ-HOSTSWITCH (Wrysk 2026-10-10: an outgrown larva looks for a bigger host).
        r("fauna.parasite_switch", "switch", Range::fraction(), "LJ-HOSTSWITCH: a rider that outgrew its host below drop-off size looks for one that bears it before going free (false: it goes free)")
            .grade("N").prov("decision").optional(),
        r("fauna.parasite_switch_radius_m", "m", Range::nonneg(), "LJ-HOSTSWITCH: how far an outgrown rider senses a host that bears it and walks to it")
            .search(1.0, 8.0).grade("N"),
        r("fauna.parasite_switch_give_up_s", "s", Range::nonneg(), "LJ-HOSTSWITCH: how long an outgrown rider looks for a new host before going free")
            .search(60.0, 3600.0).log().grade("N"),
        r("fauna.lineage_body_ceiling", "bodies", Range::nonneg(), "COMPUTE GUARD, not ecology: living bodies a lineage may hold before breeding is refused and logged (0 off)")
            .grade("N").prov("decision"),
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
        p("awareness_sensed", "fraction", fraction, "D5 (S2): a live lineage's prey whose record holds the striker dodges at this awareness"),
        p("awareness_unaware", "fraction", fraction, "D5 (S2): ...and one whose record does not, at this"),
        p("aware_within_s", "s", nonneg, "D5 (S2): ...sensed within this long before contact"),
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
        p("carcass_yield_radius_m", "m", nonneg, "a feeding lanternjaw leaves its carcass to a body that hunts it this near").search(1.0, 6.0).prov("decision"),
        p("threat_clear_m", "m", nonneg, "a yielding lanternjaw walks away until no body that hunts it is this near").search(2.0, 10.0).prov("decision"),
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
        l("wall_friction", "fraction", fraction, "share of a walk's along-wall component a wall takes away (0 slides freely, 1 stops)").search(0.0, 1.0),
        l("shallows_search_m", "m", nonneg, "a heuristic walker in water deeper than it wades heads for the nearest wadeable face within this (a world query, not a sense)"),
        l("ecological_day_s", "s", pos, "the day per-day physiology is written in (daily need, a rider's take); fixed, not the picture clock")
            .prov("decision"),
    ]);

    // `fauna.cue.*`.
    let c = |path: &str, unit: &'static str, range: Range, what: &'static str| {
        r(&format!("fauna.cue.{path}"), unit, range, what)
    };
    rows.extend([
        c("emit_reference", "organic units", pos, "detritus stock at which a scent source emits at full strength").grade("C"),
        c("half_life_s", "s", pos, "cue half-life").search(0.5, 60.0).log().grade("C"),
        c("fungal_half_life_s", "s", pos, "the fungal field's half-life (the capgnawer's Chem(fungal)): a longer scent reaches farther groves")
            .search(2.0, 1200.0).log(),
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
        t("bellwing_cruise_altitude", "fraction of band", fraction, "the height the bellwing searches at").search(0.1, 0.9),
        t("bellwing_altitude_gain", "effort per band fraction", nonneg, "lift effort per unit of altitude error").search(1.0, 10.0),
        t("bellwing_bloom_gate", "fraction of rays", fraction, "a fan sector sees blooms above this fraction of its rays").search(0.005, 0.2).log(),
        t("bellwing_bloom_near", "1 - d/range", fraction, "a bloom this near: slow and come down to it").search(0.3, 0.9),
        t("bellwing_gaze_turn", "effort", fraction, "turn effort toward the sector with the most bloom").search(0.2, 1.0),
        t("bellwing_wander_forward", "effort", fraction, "forward effort of the bellwing's no-signal wander").search(0.2, 1.0),
        t("bellwing_roost_ceiling", "fraction of band", fraction, "going to rest, cover closer than this overhead is an underside to rise into").search(0.1, 0.8),
        t("forage_stay_forward", "effort", fraction, "the blind and browser foragers' forward effort while they taste food (0 stays on food, 1 walks across it)")
            .search(0.0, 1.0).prov("decision"),
        t("forage_turn_back_samples", "samples", count, "samples the blind forager turns after leaving an exhausted patch (area-restricted search)")
            .search(0.0, 8.0),
        t("capgnawer_stay_worth", "fraction of a full cap bite", fraction, "the capgnawer holds still to feed only on food whose bite tastes at least this worth; weaker food it eats on the move (CAPGNAWER-CAPS)")
            .search(0.05, 0.9),
        t("press_replan_s", "s", nonneg, "a refused reflex walk, stalled fruit approach or hungry vine stance without reachable fruit re-plans after this long without progress (EDGE)").search(1.0, 10.0),
        t("press_shun_s", "s", nonneg, "a target given up for pressing stays given up this long").search(10.0, 600.0).log(),
        t("route_refresh_s", "s", pos, "the walking route graph a reflex walk plans on is rebuilt after this").search(10.0, 600.0).log(),
        t("route_detour", "ratio", pos, "a target is in reach when its route is at most this times the reflex's search radius").search(1.0, 4.0),
        t("way_on_m", "m", nonneg, "how far round itself a cornered body off its route graph looks for its own way back onto it").search(1.0, 8.0),
        t("give_up_fraction", "fraction of the fill rate", nonneg, "a heuristic body still-feeding gives the patch up when its bites' trailing offer falls under this share of the intake that fills its stomach (0 off)").search(0.1, 1.0),
        t("give_up_min_s", "s", nonneg, "the least time on a patch before it may be given up").search(5.0, 120.0).log(),
        t("give_up_window_s", "s", nonneg, "time constant of the trailing mean of a stay's bite offers").search(2.0, 60.0).log(),
        t("give_up_shun_s", "s", nonneg, "how long a patch given up is avoided").search(30.0, 1800.0).log(),
        t("give_up_zone_bl", "body lengths", nonneg, "a stay continues, and a given-up patch is avoided, within this of where the stay began").search(0.5, 6.0),
        t("give_up_leave_s", "s", nonneg, "after giving a patch up, any feeding answer of the heuristic becomes a walk straight on for this long").search(0.5, 15.0).log(),
        t("search_patience_s", "s", nonneg, "this long after its last taste of food, a hungry heuristic forager's own no-signal search gives way to legs away from ground it has covered (HUNGRY-SEARCH)").search(5.0, 300.0).log(),
        t("search_leg_bl", "body lengths", nonneg, "one hungry-search leg: the net way it holds a heading for (0 turns the search off)").search(3.0, 40.0).log(),
        t("search_cell_bl", "body lengths", pos, "the hungry search's visited-ground memory cell").search(0.5, 6.0),
        t("search_memory_half_life_s", "s", pos, "half-life of the hungry search's visited-ground memory").search(30.0, 1800.0).log(),
        t("lead_give_up_bl", "body lengths", nonneg, "a hungry heuristic body that has followed leads this many of its lengths at cruise since its last taste gives the lead up: the light or scent source is marked, its own lead becomes a search leg (LEADS-PAY; 0 off)").search(30.0, 600.0).log(),
    ]);

    // `fauna.hunt.*`: the lanternjaw's patrol (the detritus web D2), every one a plausible
    // starting value, untuned.
    let hk = |path: &str, unit: &'static str, range: Range, what: &'static str| {
        r(&format!("fauna.hunt.{path}"), unit, range, what)
    };
    rows.extend([
        // `patrol` (true / false) is a switch, not a number: no row.
        hk("cue_scan_m", "m", nonneg, "how far around itself a patroller reads its prey's food to choose the next patch").search(2.0, 12.0),
        hk("patch_m", "m", pos, "food cells are pooled into square patches this wide").search(0.5, 3.0),
        hk("patch_min_hop_m", "m", nonneg, "the nearest a next patch may be").search(0.5, 4.0),
        hk("patch_arrive_m", "m", nonneg, "it has arrived on a patch within this of its centre").search(0.2, 1.0),
        hk("patch_wait_s", "s", nonneg, "how long it waits still on a patch before leaving it as fruitless").search(5.0, 120.0).log(),
        hk("patrol_give_up_s", "s", nonneg, "a walk to a patch is given up after this, and the patch remembered as fruitless").search(15.0, 300.0).log(),
        hk("fruitless_memory_s", "s", nonneg, "how long a fruitless patch is avoided").search(120.0, 14400.0).log(),
        hk("fruitless_radius_m", "m", nonneg, "a patch this near a remembered fruitless one is that one").search(0.3, 3.0),
        hk("patrol_explore", "fraction", fraction, "share of patch choices made uniformly among the open patches").search(0.0, 0.6),
        hk("sight_m", "m", nonneg, "how far it sees a prey body ahead of it and not hidden").search(0.5, 3.0),
        hk("tremor_turn", "tremor units", nonneg, "the faintest prey tremor (left + right, size-eligible prey) it turns toward").search(1e-4, 0.05).log(),
        hk("search_memory_s", "s", pos, "e-fold time of the search image's encounter memory").search(120.0, 7200.0).log(),
        // `share_switching`, `strike_chosen_only`, and LJ-HUNT's `wait_strikes` and
        // `lunge_sweeps_band` (true / false) are switches: no rows.
        hk("switch_exponent", "exponent", pos, "share switching: the attack preference is the relative encounter share to this power").search(1.0, 4.0),
        hk("attack_prior_s", "s of contact", nonneg, "share switching: contact credited to every listed lineage before the shares are taken").search(1.0, 120.0).log(),
        hk("attack_half_s", "s of contact", pos, "D2's rule: encounter memory at which the attack preference is halfway to 1").search(5.0, 600.0).log(),
        hk("attack_explore", "fraction", fraction, "D2's rule: the attack preference for a prey it has not met lately (1: opportunistic)").search(0.0, 1.0),
        hk("attack_decision_s", "s", nonneg, "one attack decision on one prey stands this long").search(2.0, 120.0).log(),
        // `meal_first` and `reachable_only` (true / false) are switches: no rows.
        hk("carcass_scan_m", "m", nonneg, "how far a hungry hunter looks for a usable carcass it can walk to and eat").search(1.0, 8.0),
        hk("route_refresh_s", "s", pos, "how long a lineage's walking route graph stands before it is rebuilt").search(10.0, 600.0).log(),
        hk("success_memory_s", "s", nonneg, "how long a kill marks its place productive in the killer's memory").search(120.0, 14400.0).log(),
        hk("success_boost", "fraction", nonneg, "a patch by a remembered kill scores 1 + this times its food cue").search(0.0, 4.0),
        hk("patch_scent_weight", "weight", nonneg, "S2: its prey's scent on a square's food faces is one more patch cue at this weight (0: no scent)"),
    ]);

    // `fauna.perception.*`: the perception core (SENSES-CORE).
    let pk = |path: &str, unit: &'static str, range: Range, what: &'static str| {
        r(&format!("fauna.perception.{path}"), unit, range, what)
    };
    rows.extend([
        pk("remote_hz", "Hz", pos, "how often a body's remote senses (sight, hearing, the pair's tremor) run, staggered by id").search(1.0, 20.0),
        pk("max_candidates", "bodies", nonneg, "prey and threats weighed (one terrain ray each) per observer per remote sensing, nearest first").search(2.0, 32.0),
        pk("bin_m", "m", pos, "the spatial bins bodies are sorted into each tick").search(0.5, 8.0),
        pk("occlusion", "fraction", fraction, "hearing and tremor through terrain are multiplied by this (sight is stopped)").search(0.0, 1.0),
        pk("hearing_speed_exp", "exponent", nonneg, "hearing's emission is speed^this x length^size_exp").search(0.5, 3.0),
        pk("hearing_size_exp", "exponent", nonneg, "the length exponent of hearing's emission").search(0.0, 3.0),
        pk("hearing_near_m", "m", pos, "received loudness is emission over the distance, read as at least this"),
        pk("place_memory_fade_s", "s", nonneg, "time for last-used home-place freshness to fall linearly to zero without a physical revisit; 0 disables recall").search(60.0, 7200.0).log(),
        pk("give_up_s", "s", nonneg, "a body not sensed again within this is given up; a pursuit walks to its last-known point until then").search(1.0, 60.0).log(),
    ]);

    // `fauna.scent.*`: the scent field (SENSES-CORE).
    let sk = |path: &str, unit: &'static str, range: Range, what: &'static str| {
        r(&format!("fauna.scent.{path}"), unit, range, what)
    };
    rows.extend([
        sk("cell_m", "m", pos, "the scent field's cell (rounded to whole voxels): the connected surface faces in a cell are one node").search(0.25, 2.0),
        sk("half_life_s", "s", pos, "scent's half-life").search(5.0, 1200.0).log(),
        sk("diffuse_fraction", "fraction per update", fraction, "the share of a node's difference to its neighbours mixed in per 0.5 s update").search(0.05, 1.0),
        sk("threshold", "scent units", nonneg, "levels below this are dropped after an update"),
        sk("deposit_per_s", "scent units per organic unit per s", nonneg, "what a standing body lays, by its organic matter").search(0.1, 10.0).log(),
        sk("carrion_per_s", "scent units per organic unit per s", nonneg, "what carrion lays, by its stock").search(0.1, 10.0).log(),
        sk("step_m", "m", nonneg, "neighbouring surface faces connect within this height difference, with open air over the lower one").search(0.0, 2.0),
        sk("nectar_per_s", "scent units per s", nonneg, "what a flower holding nectar_full nectar units gives off at its bloom (the nectar plume in the air); any flower in proportion to its nectar").search(0.1, 10.0).log(),
        sk("nectar_full", "nectar units held by a full flower", nonneg, "the reference nectar stock for a flower giving off nectar_per_s; when not positive, every flower gives the full rate"),
    ]);

    // `fauna.light.*`: light and flight (S3).
    let lk = |path: &str, unit: &'static str, range: Range, what: &'static str| {
        r(&format!("fauna.light.{path}"), unit, range, what)
    };
    rows.extend([
        lk("flower_reflectance", "brightness per unit light", nonneg, "a full nectar flower's brightness (near-IR) per unit of the light falling on it").search(0.1, 4.0),
        lk("nectar_nir_share", "fraction of a full flower's reflectance", fraction, "FLOWER-FRUIT: nectar reflects near-IR; reflectance x (1 - share + share x fill), a drained flower dimmer"),
        lk("glowcap_emission", "brightness", nonneg, "a glowcap's own light").search(0.0, 2.0),
        lk("lure_emission", "brightness", nonneg, "a fully lit lure's own light (cyan)").search(0.0, 4.0),
        lk("lure_scent", "x nectar_per_s", nonneg, "BELLWING: a lit lure gives off this x a flower's nectar scent at its lure point (0: light alone)").search(0.0, 2.0),
        lk("lure_nir", "x flower_reflectance", nonneg, "BELLWING: a fully lit lure's near-IR, x a flower's; a near-IR eye reads the lure as a flower (near-IR class), 0: cyan only").search(0.0, 2.0),
        lk("background_floor", "light", pos, "a beacon's contrast is its brightness over the light around it plus this (the darkest background)").search(0.01, 0.5).log(),
        lk("half_m", "m", pos, "a beacon's received level halves at this distance").search(0.25, 4.0),
        lk("lure_height_fraction", "x body height", nonneg, "the lure hangs this far over the lanternjaw's standing surface, at its strike capsule's middle"),
        lk("visit_m", "m", nonneg, "a body this close (horizontally) to a beacon is inspecting it").search(0.1, 1.0),
        lk("visit_s", "s", nonneg, "an inspection with no meal by then fails").search(0.5, 20.0).log(),
        lk("visit_leave_m", "m", nonneg, "an open inspection whose body is farther than visit_m + this from its beacon is closed as failed").search(0.1, 2.0),
        lk("mark_s", "s", nonneg, "a failed visit marks the place 'no reward here' for this long").search(10.0, 1800.0).log(),
        lk("mark_radius_m", "m", nonneg, "a mark covers beacons within this of its centre").search(0.1, 2.0),
        lk("lift_per_m", "lift per m", nonneg, "a flyer approaching a beacon climbs or drops toward its height at this lift per metre"),
        lk("near_m", "m", nonneg, "inside this of a beacon it slows to near_forward; walkers also start the no-reward inspection timer"),
        lk("near_forward", "fraction", fraction, "a flyer's forward effort near a beacon"),
        lk("glow_scent_gate", "chem response", fraction, "a hungry ground body is drawn to a still cyan light only while its food chemistry response is below this").search(0.0, 0.5),
        // S3b: which light pays (`fauna.light.learn` is a switch: no row) and looming
        // (LJ-HUNT's `fauna.light.lure_masks_loom` is a switch: no row).
        lk("nir_innate", "probability", fraction, "S3b: a naive flyer's reward expectation of a near-IR reflector (flowers); learned ones relax back to it"),
        lk("cyan_innate", "probability", fraction, "S3b: a naive flyer's reward expectation of a cyan emitter (glowcaps, lures)"),
        lk("learn_rate", "fraction", fraction, "S3b: a sip (1) or a failed visit (0) moves its class's expectation this fraction of the way").search(0.05, 1.0),
        lk("forget_s", "s", pos, "S3b: a learned expectation relaxes back to innate with this time constant").search(300.0, 21600.0).log(),
        lk("expect_min", "probability", fraction, "S3b: a still beacon whose class is expected to pay less than this is not approached").search(0.0, 0.5),
        lk("loom_tau_s", "s", nonneg, "S3b: a moving body looms only while it would reach the eye within this (distance / closing speed; 0: any moving body)").search(0.2, 3.0),
    ]);

    // `fauna.founding.*`: the seeder's founders.
    let f = |path: &str, unit: &'static str, range: Range, what: &'static str| {
        r(&format!("fauna.founding.{path}"), unit, range, what)
    };
    rows.extend([
        f("arrival_body", "fraction of adult body", fraction, "a seeded founder arrives at this fraction of its adult body").search(0.3, 1.0).prov("decision"),
        f("arrival_reserve", "fraction of reserve cap", fraction, "a seeded founder arrives with this fraction of its reserve cap").search(0.0, 1.0).prov("decision"),
        f("arrival_stomach", "fraction of stomach capacity K", fraction, "a seeded founder arrives with this fraction of its stomach capacity, of its first food").search(0.0, 1.0).prov("decision"),
        f("starter_litter_organic", "organic units", nonneg, "leaf litter laid under each seeded littershredder").search(0.05, 1.0),
        f("starter_litter_mineral_fraction", "fraction of organic", fraction, "mineral share of the starter litter").search(0.005, 0.1),
        f("starter_litter_energy_density", "energy per organic unit", nonneg, "retained energy density of the starter litter").search(0.5, 4.0),
        f("accept_founder_hours", "founder-hours", nonneg, "founder-hours of adult upkeep a component's stock must cover per founder").search(0.1, 6.0),
        f("clearance_margin_m", "m", nonneg, "slack added to the founder's disc radius when choosing where founders may be placed"),
        f("reference_land_m2", "m2", nonneg, "dry support area the founding counts are written for; positive scales counts and plant founder caps by the world's own area (0: counts as written)").prov("decision"),
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
