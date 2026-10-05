//! The plant layer's parameter table (`flora.*`, package R1): one row per numeric field of
//! [`crate::FloraConfig`], the [`crate::SpeciesConfig`] rows mounted once per species, the
//! latticevine's ([`crate::VineConfig`]) and the layer-wide ones. Units, grades, provenance
//! and search ranges are the audit's `parameters.tsv` where it has a row. The generic
//! code is [`cubarium_voxel::registry`].
//!
//! The **scale tags** are the voxel rescale ([`crate::FloraConfig::rescaled_to`]): each
//! species' root box, mycelium reach and hop are distances counted in reference voxels
//! (`Scale::Length`), and the latticevine's per-face amounts follow a face's area
//! (`Scale::Area`). Everything else is a rate, a fraction or metres and stays.

use cubarium_voxel::registry::{Alias, Group, GroupKind, Range, Registry, Row, Scale, Tie};

use crate::{FloraConfig, MAX_FOLIAGE_LAYERS, Species, SpeciesConfig, VineConfig};

/// The plant layer's table.
pub fn registry() -> &'static Registry {
    static TABLE: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        Registry::new(
            "flora",
            rows(),
            vec![Alias {
                pattern: "flora.*.propagule_split".into(),
                names: vec!["w_frac", "p_frac", "q_frac"],
            }],
            Vec::new(),
        )
        .with_ties(ties())
        .with_groups(groups())
    })
}

/// The plant layer's `derive()` (registry §4): a seed bank's attrition is one over its
/// age limit, and dead wood and carrion rot with the litter (today's ratios kept).
pub fn ties() -> Vec<Tie> {
    vec![
        Tie::inverse("seed_attrition", "flora.*.seed_attrition_per_s", "flora.*.seed_max_age_s"),
        Tie::new("wood_decay_follows_litter", "flora.wood_decomposition", "flora.decomposition"),
        Tie::new("carrion_decay_follows_litter", "flora.carrion_decomposition", "flora.decomposition"),
    ]
}

/// The plant groups a search names, and the plant layer's share of the biological clock.
/// - `@photo`: every photosynthetic species (the glowcap is a fungus).
/// - Functional groups (audit §6): `@turf` the pioneer herbs, `@mat` the low covers,
///   `@shrub`, `@tree`.
/// - `@bio_rate` / `@bio_time` (`K_bio`, registry decision 2), derived fields left out.
fn groups() -> Vec<Group> {
    let seg = |name: &'static str, members: &[Species]| Group {
        name,
        kind: GroupKind::Segment(members.iter().map(|s| s.name().to_string()).collect()),
    };
    let paths = |name: &'static str, members: &[&str]| Group {
        name,
        kind: GroupKind::Paths(members.iter().map(|s| s.to_string()).collect()),
    };
    use Species::*;
    vec![
        seg(
            "photo",
            &[Bloomcrown, Umbrellafrond, Springturf, Stonecushion, Velvetpad, Vaulttree, Lanternberry, Siphonreed],
        ),
        seg("turf", &[Springturf, Siphonreed]),
        seg("mat", &[Velvetpad, Stonecushion]),
        seg("shrub", &[Bloomcrown, Umbrellafrond, Lanternberry]),
        seg("tree", &[Vaulttree]),
        paths(
            "bio_rate",
            &[
                "flora.*.foliage_rate",
                "flora.*.wood_rate",
                "flora.*.assimilation",
                "flora.*.maintenance",
                "flora.*.senescence",
                "flora.*.propagule_rate",
                "flora.*.substrate_uptake_per_s",
                "flora.decomposition",
                "flora.latticevine.leaf_upkeep",
                "flora.latticevine.wood_upkeep",
                "flora.latticevine.regrow_rate",
            ],
        ),
        paths(
            "bio_time",
            &[
                "flora.*.seed_max_age_s",
                "flora.*.defence_half_life_s",
                "flora.latticevine.dry_spell_s",
                "flora.latticevine.wet_spell_s",
                "flora.latticevine.bud_s",
                "flora.latticevine.flower_s",
                "flora.latticevine.fruit_s",
                "flora.latticevine.spent_s",
                "flora.latticevine.seed_max_age_s",
            ],
        ),
    ]
}

fn r(path: &str, unit: &'static str, range: Range, what: &'static str) -> Row {
    Row::new(path.to_string(), unit, range, what)
}

/// [`SpeciesConfig`]'s rows, relative to `flora.<species>`.
fn species_rows() -> Vec<Row> {
    let nonneg = Range::nonneg();
    let fraction = Range::fraction();
    // Read only as bounds by the old validator, which asked of them only finiteness.
    let finite = Range::any();
    vec![
        r("substrate_uptake_per_s", "organic/s per unit mycelium", nonneg, "saprotroph: dead wood and litter withdrawn")
            .evolve().log().grade("C"),
        r("substrate_yield", "fraction", fraction, "saprotroph: share of the substrate taken that becomes tissue")
            .search(0.2, 0.7).grade("A"),
        r("establish_substrate_min", "org-units", nonneg, "saprotroph: substrate a spore cohort needs in its box").grade("C"),
        r("substrate_reach_up_down", "reference voxels (0.25 m)", nonneg, "saprotroph: the mycelium box's vertical reach")
            .scale(Scale::Length).evolve().grade("C"),
        r("alpha", "foliage per unit wood", nonneg, "foliage cap P_cap = alpha * W").search(0.05, 3.0).log().grade("D"),
        r("wood_max", "org-units", Range::positive(), "the most living wood one stand holds").evolve().log().grade("D"),
        r("reserve_cap", "reserve per unit wood", nonneg, "Q_max = q_cap * W").search(0.1, 2.0).log().grade("B"),
        r("maintenance", "/s per unit wood", nonneg, "wood upkeep m_w").evolve().log().grade("B"),
        r("senescence", "/s of foliage", nonneg, "leaf fall m_p, to litter").evolve().log().grade("B"),
        r("foliage_rate", "organic/s per unit wood", nonneg, "foliage regrowth cap r_p").evolve().log().grade("D"),
        r("wood_rate", "/s per unit wood", nonneg, "wood growth cap r_w").evolve().log().grade("C"),
        r("build", "fraction", nonneg, "construction respiration c_g: a unit built costs 1 + c_g")
            .search(0.1, 0.5).log().grade("A"),
        r("dieback", "wood per unit unpaid upkeep", nonneg, "kappa: wood lost per unit maintenance not paid")
            .search(0.3, 3.0).log().grade("C"),
        r("alive_min", "org-units", nonneg, "W_min: a stand with less wood dies").evolve().grade("C"),
        r("donor_min", "org-units of wood", nonneg, "wood needed before sending propagules").grade("C"),
        r("donor_reserve_floor", "fraction of Q_max", nonneg, "reserve a donor keeps back").search(0.0, 0.9).grade("C"),
        r("propagule_rate", "organic/s gross", nonneg, "reserve set aside for seed per second").evolve().log().grade("D"),
        r("propagule_split.*", "fraction", nonneg, "how a landed package splits into wood, foliage and reserve (sums to 1)")
            .search(0.1, 0.8).grade("C"),
        r("energy_density", "energy per org-unit", nonneg, "e_v of plant tissue").grade("D"),
        r("seed_attrition_per_s", "/s", nonneg, "a banked seed's death rate").grade("C").prov("derived"),
        r("seed_max_age_s", "s", nonneg, "dormancy: a bin older than this falls to litter").evolve().log().grade("B"),
        r("seed_cohorts_max", "bins", Range::at_least(1.0), "the bank's age resolution").grade("N").prov("numerics"),
        r("n_tissue", "mineral per org-unit", nonneg, "stoichiometry of built tissue").search(0.005, 0.05).log().grade("B"),
        r("reserve_share", "fraction", nonneg, "share of a tick's surplus to the reserve first").search(0.0, 0.6).grade("C"),
        r("reflush_below", "fraction of P_cap", nonneg, "reserve funds foliage only below this").search(0.0, 1.0).grade("C"),
        r("seedling_below", "multiple of alive_min", nonneg, "a seedling's reflush answers to its seed's leaf, not alpha, below this wood")
            .search(1.0, 5.0).grade("C"),
        r("assimilation", "organic/s per unit foliage", nonneg, "g: gross fixation at full light and moisture")
            .evolve().log().grade("B"),
        r("nutrient_half", "mineral units", nonneg, "K_N: mineral half-saturation").search(0.1, 2.0).log().grade("C"),
        r("nutrient_draw_max", "/s per unit site mineral", nonneg, "f_max: rate cap on income from the site's mineral")
            .search(0.003, 0.1).log().grade("D"),
        r("light_half", "fraction of open sky", nonneg, "light response half-saturation").search(0.05, 1.5).grade("B"),
        r("rooting_depth", "reference voxels (0.25 m)", nonneg, "root box depth below the support")
            .scale(Scale::Length).evolve().grade("C"),
        r("rooting_radius", "reference voxels (0.25 m)", nonneg, "root box half-width")
            .scale(Scale::Length).grade("C"),
        r("wilt_water", "available water (0 wp, 1 fc)", finite, "mu = 0 at and below").evolve().grade("B").prov("measured"),
        r("full_water", "available water", finite, "mu = 1 at and above").evolve().grade("B").prov("measured"),
        r("transpiration_m3_per_s", "m3/s per unit foliage at mu = 1", nonneg, "water drawn from the root box")
            .grade("D").prov("measured"),
        r("establish_water_min", "available water", finite, "germination moisture gate").evolve().grade("B"),
        r("establish_light_min", "sky visibility", finite, "germination light gate").evolve().grade("C"),
        r("saturated_pore", "pore fraction of capacity", finite, "a root voxel at or above this is saturated")
            .search(0.8, 1.0).grade("B"),
        r("stress_rate_per_s", "/s fraction of gap", nonneg, "aeration stress rise").search(0.001, 0.2).log().grade("D"),
        r("relax_rate_per_s", "/s fraction of gap", nonneg, "aeration stress recovery").evolve().grade("B"),
        r("establish_saturated_max", "fraction of root box saturated", finite, "germination ceiling and adult stress onset")
            .evolve().grade("C"),
        r("drown_depth_m", "m of water", nonneg, "standing water over the support that kills the stand")
            .evolve().grade("D").prov("tuned"),
        r("water_depth_min_m", "m of settled standing water", nonneg, "needed on or beside the site").evolve().grade("B"),
        r("hop", "reference voxels (0.25 m)", nonneg, "dispersal radius / wind kernel scale")
            .scale(Scale::Length).evolve().log().grade("D"),
        r("clonal_share", "fraction of packages", fraction, "runner/rhizome share").search(0.0, 1.0).grade("C").prov("tuned"),
        r("spore_tries", "tries", Range::at_least(1.0), "a spore package's landing attempts").evolve().grade("C").prov("tuned"),
        r("seed_mass", "minimum packages", Range::at_least(1.0), "seed size and lottery weight").evolve().grade("D"),
        r("overtop_per_check", "probability per check", fraction, "a seedling comes up through a ground cover overtop_covers allows")
            .evolve().grade("C").prov("tuned"),
        // A list of species names, not a number: the row documents it, and no numeric
        // leaf ever matches it (hence optional). Empty = any ground cover (the woody rule).
        r("overtop_covers", "species names ([] = any ground cover)", finite, "the ground covers a seedling may come up through")
            .optional().grade("N").prov("decision"),
        r("crown_height_m.*", "m", nonneg, "crown top above the support [min, max]").evolve().grade("B"),
        r("crown_radius_m.*", "m", nonneg, "crown half-width [min, max]").evolve().grade("B"),
        r("graze_refuge", "fraction of each layer's capacity", Range::closed(0.0, 1.0).open_hi(), "foliage no bite can take")
            .search(0.0, 0.6).grade("B"),
        r("defence_per_bite", "fraction toward 1 per bite", fraction, "induced defence rise")
            .evolve().log().grade("D").prov("measured"),
        r("defence_half_life_s", "s (<= 0: no decay)", finite, "induced defence half-life once grazing stops")
            .evolve().log().grade("D"),
        r("nectar_per_s", "org-units/s per stand", nonneg, "reserve an adult stand moves into nectar per second, in daylight")
            .grade("N").prov("decision"),
        r("nectar_cap", "org-units per stand", nonneg, "the most nectar an adult stand holds").grade("N").prov("decision"),
        r("nectar_mineral_fraction", "fraction of organic", fraction, "mineral a unit of nectar carries (pollen lumped in)").grade("N").prov("decision"),
        r("fruit_per_s", "org-units/s per stand", nonneg, "reserve an adult stand moves into hanging fruit per second, in daylight")
            .grade("N").prov("decision"),
        r("fruit_cap", "org-units per stand", nonneg, "the most fruit an adult stand holds").grade("N").prov("decision"),
        r("fruit_drop_s", "s", nonneg, "ripe fruit left uneaten this long falls to the litter at the stand's site").grade("N").prov("decision"),
        r("fruit_rest_s", "s", nonneg, "mean rest (exponential, per stand) after an uneaten crop falls before fruit hangs again; 0 none")
            .search(0.0, 3600.0),
        r("fruit_mineral_fraction", "fraction of organic", fraction, "mineral a unit of fruit carries (small, not zero)").grade("N").prov("decision"),
        r("night_bloom_scale", "fraction of the day rate", fraction, "nectar and fruit made at night out of the night budget")
            .search(0.0, 1.0).grade("N").prov("decision"),
        r("night_floor", "fraction of reserve cap", fraction, "the reserve a stand keeps for dawn; the night budget spends only above it")
            .search(0.0, 0.8).grade("N").prov("decision"),
        r("night_min_s", "s", Range::at_least(1.0), "the night budget is spread over at least this long, so it stays bounded near dawn")
            .search(60.0, 3600.0).log().grade("N").prov("numerics"),
        r("runner_reach", "voxels (not rescaled)", nonneg, "columns sideways a runner daughter may root from its parent")
            .search(1.0, 3.0),
        r("runners_overtop", "switch (true / false)", finite, "a runner may root on a ground cover overtop_covers allows, displacing it")
            .optional().grade("N").prov("decision"),
        r("dispersal", "mode (Drop, Wind, Spores, Water, Wald)", finite, "how the species' seeds travel")
            .optional().grade("N").prov("decision"),
        r("wald_mean_m", "m", nonneg, "Wald dispersal: the WALD (inverse-Gaussian) kernel's mean distance, the wind knob")
            .search(0.25, 6.0).log(),
        r("wald_shape_m", "m", nonneg, "Wald dispersal: the WALD kernel's shape lambda; CV = sqrt(mean / lambda)")
            .search(0.1, 4.0).log(),
        r("burr_pickup", "probability per contact at coat 1", fraction, "burrs: a donor's seed latches onto an animal stepping onto its face")
            .search(0.0, 0.5),
        r("burr_retention_mean_m", "m walked", nonneg, "burrs: mean distance a seed rides before it falls off (Weibull)")
            .search(0.5, 16.0).log(),
        r("burr_retention_shape", "-", Range::positive(), "burrs: Weibull shape k of the retention curve (1 exponential, < 1 a falling detachment rate)")
            .search(0.3, 2.0),
        r("cure_water", "available water (0 wp, 1 fc)", nonneg, "curing: the root box counts as dry below this")
            .search(0.0, 0.5),
        r("cure_after_s", "s (0 = never cures)", nonneg, "curing: dry this long, the stand drops its foliage and goes dormant")
            .search(0.0, 7200.0),
        r("cure_shed", "fraction of foliage", fraction, "curing: share of the foliage dropped to the litter at once")
            .search(0.0, 1.0),
        r("cure_upkeep", "fraction of maintenance", fraction, "curing: a dormant stand's maintenance against its waking rate")
            .search(0.0, 1.0),
        r("cure_resume_water", "available water", nonneg, "curing: a dormant stand wakes and reflushes at and above this")
            .search(0.0, 1.0),
        r("cure_max_s", "s (0 = no limit)", nonneg, "curing: dormant this long, the drought kills the stand")
            .search(0.0, 86400.0),
        r("cured_srgb", "sRGB 0x00RRGGBB (look, 0 = the species' palette)", nonneg, "the plain-voxel colour of a cured stand's crown")
            .optional().grade("N").prov("decision"),
        r("founder_per_m2", "founders per m2 of habitat", nonneg, "founding: stands per square metre of ground the species' own gates pass on")
            .search(0.0, 10.0),
        r("founder_colony", "stands", Range::at_least(1.0), "founding: the size of one founder colony").search(1.0, 12.0),
        r("founder_cap", "stands", nonneg, "founding: the cap a world puts on this species' founders").search(0.0, 400.0),
        r("dark_sky_max", "sky visibility (0 = no dark habitat)", fraction, "dark habitat: a site at or above this sky is not dark")
            .search(0.0, 0.8),
        r("dark_sky_full", "sky visibility", fraction, "dark habitat: a site at or below this sky is fully dark")
            .search(0.0, 0.4),
        r("dark_uptake_per_s", "organic/s per unit mycelium at darkness 1", nonneg, "dark habitat: seep income, with its own mineral")
            .search(0.0, 0.03).grade("D"),
        r("dark_damp", "available water (0 wp, 1 fc)", nonneg, "dark habitat: least available water a fully dark site reads")
            .search(0.0, 1.0).grade("D"),
        r("dark_establish_min", "darkness (0 = off)", fraction, "dark habitat: darkness that opens the substrate gate with no substrate")
            .search(0.0, 1.0),
        r("dark_spore_bias", "weight per unit darkness", nonneg, "dark habitat: spore landings weigh 1 + bias * darkness")
            .search(0.0, 10.0),
        r("dark_founder_share", "fraction of founders", fraction, "founding: share of founders placed on the darkest faces, no log laid")
            .search(0.0, 1.0),
        r("profile.*.wood_fraction_max", "fraction of wood_max (inf: the last stage)", Range::nonneg_or_inf(), "a stage applies up to this wood fraction")
            .evolve().grade("C"),
        r("profile.*.height_m_max", "m", Range::positive(), "a stage's crown-height ceiling (a capped seedling)")
            .optional().evolve().grade("B"),
        r("profile.*.layers.*.band.*", "fraction of crown height", Range::fraction(), "a layer's band [bottom, top]")
            .evolve().grade("C"),
        r("profile.*.layers.*.radius", "fraction of crown radius", Range::closed(0.0, 1.0).open_lo(), "a layer's radius")
            .grade("C"),
        r("profile.*.layers.*.share", "fraction of the stage's foliage", nonneg, "a foliage layer's share (a trunk's is 0)")
            .evolve().grade("C"),
        r("profile.*.layers.*.porosity", "fraction transmitted", Range::fraction(), "light a layer lets through")
            .evolve().grade("C"),
    ]
}

/// [`VineConfig`]'s rows, relative to `flora.latticevine`.
fn vine_rows() -> Vec<Row> {
    let nonneg = Range::nonneg();
    let positive = Range::positive();
    let area = Scale::Area;
    vec![
        r("rooting_depth", "reference voxels (0.25 m)", nonneg, "root box depth").scale(Scale::Length).evolve().grade("C"),
        r("rooting_radius", "reference voxels (0.25 m)", nonneg, "root box radius").scale(Scale::Length).evolve().grade("C"),
        r("transpiration_m3_per_s", "m3/s per reference face", nonneg, "water per full face").scale(area).grade("D").prov("derived"),
        r("wilt_water", "available water", nonneg, "mu = 0 at and below").evolve().grade("B"),
        r("full_water", "available water", nonneg, "mu = 1 at and above").evolve().grade("B"),
        r("dry_water", "available water", nonneg, "the root box is dry below").evolve().grade("C"),
        r("wet_water", "available water", nonneg, "and wet again above").evolve().grade("C"),
        r("dry_spell_s", "s", nonneg, "dry this long: dormant").evolve().grade("C"),
        r("wet_spell_s", "s", nonneg, "wet this long: regreens").evolve().grade("C"),
        r("leaf_fall_s", "s", positive, "a dormant face drops its leaf over this").grade("C"),
        r("assimilation", "organic/s per reference face", nonneg, "income per full face in full light")
            .scale(area).evolve().log().grade("C"),
        r("underside_light", "fraction", nonneg, "light factor on a downward face").evolve().grade("B"),
        r("leaf_mass", "org-units per reference face", positive, "leaf tissue per full face").scale(area).evolve().grade("C"),
        r("runner_mass", "org-units per reference face", nonneg, "runner tissue per face").scale(area).evolve().grade("C"),
        r("leaf_upkeep", "organic/s per reference face", nonneg, "leaf maintenance").scale(area).evolve().grade("B"),
        r("wood_upkeep", "organic/s per reference face", nonneg, "runner maintenance").scale(area).evolve().grade("B"),
        r("regrow_rate", "leafiness/s", nonneg, "a face's releafing rate").evolve().grade("B"),
        r("spread_cost", "org-units per reference face", nonneg, "a new face's cost (at least its runner)")
            .scale(area).evolve().grade("C"),
        r("spread_check_s", "s", positive, "how often a vine tries to spread").grade("C"),
        r("sister_cost", "org-units per reference face", nonneg, "a sister vine's cost").scale(area).evolve().grade("C"),
        r("sister_min_dist", "faces", nonneg, "a sister roots at least this far along the cover").evolve().grade("C"),
        r("contest_threshold", "leafiness", Range::fraction(), "a face below this is contestable").evolve().grade("C"),
        r("contest_check_s", "s", positive, "the contest wheel's period").grade("C"),
        r("spur_reserve_min", "org-units per reference face", nonneg, "reserve per face before a flowering cycle")
            .scale(area).evolve().grade("C"),
        r("spread_hold", "multiples of spur_reserve_min per face", nonneg, "reserve kept back from spreading and sisters for flowering; 0 spends it all")
            .search(0.0, 6.0),
        r("spur_share", "fraction of covered faces", Range::fraction(), "long-run share of faces carrying a flower or fruit at once; each face rests a random (exponential) time between cycles")
            .search(0.02, 0.5),
        r("bud_s", "s", nonneg, "bud phase").evolve().grade("C"),
        r("flower_s", "s", nonneg, "flower phase").evolve().grade("C"),
        r("fruit_s", "s", nonneg, "fruit phase").evolve().grade("C"),
        r("spent_s", "s", nonneg, "spent phase").evolve().grade("C"),
        r("nectar_per_face_s", "org-units/s per reference face", nonneg, "long-run nectar a covered face offers: production per unit of vine area (a flower holds this times the mean cycle)")
            .scale(area).evolve().log().grade("C"),
        r("fruit_per_face_s", "org-units/s per reference face", nonneg, "long-run bead fruit a covered face sets: production per unit of vine area (a bunch holds this times the mean cycle)")
            .scale(area).evolve().log().grade("C"),
        r("energy_density", "energy per org-unit", nonneg, "vine tissue energy").grade("D"),
        r("mineral_fraction", "mineral per org-unit", Range::fraction(), "vine tissue mineral: the store it draws from its root, and what its leaf, nectar and fruit carry")
            .prov("decision"),
        r("founders", "vines", nonneg, "founder vines rooted on a fresh world: the floor under the area count").evolve().grade("C"),
        r("founders_per_m2", "vines per m2 of world floor", nonneg, "founder vines per square metre: max(founders, round(this * area))")
            .search(0.0, 0.25),
        r("founder_reserve", "org-units per reference face", nonneg, "each founder's starting reserve")
            .scale(area).evolve().grade("C"),
        r("founder_spacing", "voxels (not rescaled)", nonneg, "the fewest voxels between two founders' roots")
            .search(2.0, 24.0),
        r("founder_spread_m", "m of floor", nonneg, "floor distance founders keep while the walls allow it; the rest fill at founder_spacing; 0 off")
            .search(0.0, 8.0),
        r("seed_max_age_s", "s", nonneg, "a banked latticevine seed (voided from a gut) older than this at its check falls to litter")
            .evolve().log().grade("C"),
    ]
}

fn rows() -> Vec<Row> {
    let mut rows = Vec::new();
    for s in Species::ALL {
        let at = format!("flora.{}", s.name());
        rows.extend(species_rows().into_iter().map(|row| row.under(&at)));
    }
    rows.extend(vine_rows().into_iter().map(|row| row.under("flora.latticevine")));
    let nonneg = Range::nonneg();
    let positive = Range::positive();
    let fraction = Range::fraction();
    rows.extend([
        r("flora.shade_k_per_m2", "m2 per org-unit", nonneg, "canopy attenuation per square metre").evolve().log().grade("D").prov("derived"),
        r("flora.decomposition", "/s", nonneg, "k_d: litter decomposition").evolve().log().grade("B"),
        r("flora.wood_decomposition", "/s", nonneg, "k_w: dead-wood decomposition").evolve().log().grade("C"),
        r("flora.carrion_decomposition", "/s", nonneg, "k_c: carrion decomposition").evolve().log().grade("A"),
        r("flora.litter_energy_cap", "energy per org-unit", nonneg, "retained energy cap per unit of litter").grade("C"),
        r("flora.initial_mineral", "mineral per site", nonneg, "mineral a site is provisioned with").evolve().log().grade("D"),
        r("flora.seed_check_s", "s", nonneg, "how often a banked site is tested").grade("N").prov("numerics"),
        r("flora.step_every", "ticks", Range::at_least(1.0), "the plant clock: one plant step per this many world ticks")
            .grade("N")
            .prov("numerics"),
        r("flora.upkeep_q10", "x per 10 C", positive, "Q10 of plant upkeep around the climate's mean").search(1.0, 4.0),
        r("flora.wind_tries", "tries", Range::at_least(1.0), "a wind seed's draws before the donor keeps its package")
            .search(1.0, 16.0),
        r("flora.burr_cap", "seeds per animal", nonneg, "burrs: the most seeds one animal's coat carries at once")
            .search(1.0, 20.0),
        r("flora.wind_shape", "-", positive, "wind kernel: Lomax shape k (larger is a thinner tail)").search(1.2, 4.0),
        r("flora.wind_scale_hops", "hops", positive, "wind kernel: Lomax scale a, in the species' hops").search(0.5, 8.0),
        r("flora.carcass_remains_fraction", "fraction of the body's organic matter", fraction, "carrion below this draws as remains, not the body")
            .search(0.1, 0.6),
        r("flora.carcass_gone_fraction", "fraction of the body's organic matter", fraction, "carrion at or below this drops the carcass record")
            .search(0.005, 0.1).log(),
        r("flora.seed_mark_recent_s", "s", nonneg, "how long a seed landing keeps its site's mark").search(10.0, 600.0),
        r("flora.min_layer_area_m2", "m2", positive, "floor on a foliage layer's area in the shade exponent")
            .search(0.01, 0.25),
        r("flora.founder_wood_fractions.*", "fraction of wood_max", fraction, "founding: a founder's starting wood, cycled in placement order")
            .search(0.1, 1.0),
        r("flora.founder_canopy_shade", "fraction of sky", fraction, "founding: sky a crown takes from the sites under it while placing")
            .search(0.2, 0.9),
        r("flora.founder_log_organic", "org-units", nonneg, "founding: dead wood laid under each glowcap founder").search(0.1, 1.0),
        r("flora.founder_patch_spacing_m", "m", nonneg, "founding: least distance between two patch centres").search(0.5, 4.0),
        r("flora.voxel_m", "m", Range::positive(), "the world's cell size, recorded (set from world.voxel_m)").prov("derived"),
        r("flora.film_max_depth_m", "m", nonneg, "glassfilm: deepest standing water a site is wet-eligible under"),
        r("flora.film_extinction_per_m", "/m", nonneg, "glassfilm: light extinction per metre of water"),
        r("flora.film_growth_per_s", "/s", nonneg, "glassfilm: logistic growth rate at full light"),
        r("flora.film_capacity_per_m2", "org-units per m2", nonneg, "glassfilm: carrying capacity per square metre"),
        r("flora.film_graze_refuge", "fraction of capacity", Range::fraction(), "glassfilm: share of a site's capacity a grazer cannot reach (the refuge it regrows from)")
            .search(0.0, 0.5),
        r("flora.film_maintenance_per_s", "/s", nonneg, "glassfilm: upkeep, respired"),
        r("flora.film_spread_per_s", "/s", nonneg, "glassfilm: fragments a neighbour sends per unit of its film"),
        r("flora.film_dry_death_per_s", "/s", nonneg, "glassfilm: loss on a dry or too-deep site, to litter"),
        r("flora.film_wet_min_depth_m", "m", nonneg, "glassfilm: shallowest standing water a site is wet-eligible under (a residue is dry)"),
        r("flora.film_gone_fraction", "fraction of capacity", fraction, "glassfilm: dry film below this share of capacity dies whole to litter"),
        r("flora.film_seed_per_m2", "org-units per m2", nonneg, "glassfilm: founding film per square metre"),
        r("flora.film_n_tissue", "mineral per org-unit", nonneg, "glassfilm: mineral per unit of film tissue, from the site's pool"),
        r("flora.film_energy_density", "energy per org-unit", nonneg, "glassfilm: energy per unit of film tissue"),
    ]);
    rows
}

/// The cross-field rules no single row can state: the anatomy, the propagule split, a
/// newborn that is alive, the latticevine's water and spread thresholds.
pub(crate) fn cross_rules(c: &FloraConfig) -> Result<(), String> {
    for s in Species::ALL {
        species_rules(&format!("flora.{}", s.name()), c.species(s))?;
    }
    vine_rules(&c.latticevine)
}

fn species_rules(at: &str, sc: &SpeciesConfig) -> Result<(), String> {
    let fail = |what: String| Err(format!("{at}: {what}"));
    // The anatomy, before anything else reads it: a stand's layer stocks are binned
    // against this the moment it is created, so a malformed profile is a world that
    // cannot conserve its own foliage rather than a number that looks odd later.
    if sc.profile.is_empty() {
        return fail("profile is empty: every species states its anatomy".into());
    }
    let mut previous = 0.0f64;
    for (i, stage) in sc.profile.iter().enumerate() {
        if stage.wood_fraction_max < previous {
            return fail(format!(
                "profile stage {i} threshold {} is below stage {}'s {previous}: the stages \
                 are read in order",
                stage.wood_fraction_max,
                i.saturating_sub(1)
            ));
        }
        previous = stage.wood_fraction_max;
        if stage.layers.is_empty() {
            return fail(format!("profile stage {i} has no layers"));
        }
        if stage.foliage_layer_count() > MAX_FOLIAGE_LAYERS {
            return fail(format!(
                "profile stage {i} has {} foliage layers, over the {MAX_FOLIAGE_LAYERS} a \
                 stand can hold stocks for",
                stage.foliage_layer_count()
            ));
        }
        let mut share = 0.0;
        for (j, layer) in stage.layers.iter().enumerate() {
            let [lo, hi] = layer.band;
            if hi <= lo {
                return fail(format!(
                    "profile stage {i} layer {j} band {:?} is not a rising interval of the \
                     crown height",
                    layer.band
                ));
            }
            if layer.kind.bears_foliage() {
                share += layer.share;
            } else if layer.share != 0.0 {
                return fail(format!(
                    "profile stage {i} layer {j} is a trunk holding a share of {}: structure \
                     is not foliage",
                    layer.share
                ));
            }
        }
        if (share - 1.0).abs() > 1e-12 {
            return fail(format!(
                "profile stage {i} foliage shares sum to {share}, not one: a stand's layers \
                 would not sum to its foliage"
            ));
        }
    }
    if sc.profile.last().expect("a non-empty profile").wood_fraction_max < 1.0 {
        return fail("the last profile stage must catch a full-grown stand (threshold >= 1)".into());
    }
    let [w_frac, p_frac, q_frac] = sc.propagule_split;
    let sum = w_frac + p_frac + q_frac;
    if (sum - 1.0).abs() > 1e-12 {
        return fail(format!(
            "propagule_split {:?} sums to {sum}, not one: a germination would create or \
             destroy organic matter",
            sc.propagule_split
        ));
    }
    if !(w_frac > 0.0) {
        return fail(format!(
            "propagule_split w_frac is {w_frac}: a species that puts nothing into wood has no \
             package size and can never germinate"
        ));
    }
    if sc.dark_sky_full > sc.dark_sky_max {
        return fail(format!(
            "dark_sky_full {} is over dark_sky_max {}: fully dark must be at least as dark as \
             not dark at all",
            sc.dark_sky_full, sc.dark_sky_max
        ));
    }
    if sc.alive_min > sc.wood_max {
        return fail(format!(
            "alive_min {} is over wood_max {}: every newborn would die on its first growth tick",
            sc.alive_min, sc.wood_max
        ));
    }
    Ok(())
}

fn vine_rules(vc: &VineConfig) -> Result<(), String> {
    if vc.wet_water <= vc.dry_water {
        return Err(format!(
            "flora.latticevine: wet_water {} must be above dry_water {}",
            vc.wet_water, vc.dry_water
        ));
    }
    if vc.spread_cost < vc.runner_mass {
        return Err(format!(
            "flora.latticevine: spread_cost {} cannot pay the runner_mass {} it builds",
            vc.spread_cost, vc.runner_mass
        ));
    }
    Ok(())
}
