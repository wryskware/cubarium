//! The joint ecological parameter vector this milestone searches — **ecology v1**.
//!
//! Every entry names a field that **already exists** in [`WorldConfig`]; nothing here invents
//! new biology to create a knob, and nothing here changes an equation. Bounds are hypotheses
//! around the shipped defaults (`design/ecology-v1-contract.md` §11), not established viable
//! ranges, and they are deliberately allowed to straddle the core's own validity constraints
//! so that an invalid candidate is *recorded as rejected* rather than quietly repaired (see
//! `drives.bud_reserve`). [`WorldConfig::validate`] is the only validity gate.
//!
//! Parameters that are plausible candidates but are **not** searched in this milestone are
//! listed in [`EXCLUDED`], with the reason.
//!
//! ## Why these fourteen
//!
//! The calibration brief names four axes — intake/assimilation pressure, foliage growth and
//! reserve allocation, maturation and reproductive timing, and recycling — and asks for
//! roughly 8–12 names drawn from them. This vector carries **thirteen**: eleven of the
//! brief's own names plus `producer.growth` and `producer.mortality`, the two terms that
//! decide whether a cell can hold any foliage at all.
//!
//! The arithmetic that forced the addition, from the ecology v1 steady state with both §4.4
//! growth caps applied. In a cell that is *income*-limited (the leaf cap `r_p·W` does not
//! bind), foliage settles where growth equals senescence:
//!
//! ```text
//! (1 − q_share)·(c·P − m_w·W)/(1 + c_g) = m_p·P,     c = g · L_eff · μ · N/(N + K_N)
//! ```
//!
//! which has a positive solution only when `(1 − q_share)·c/(1 + c_g) > m_p`, i.e. only above
//! a **critical light-moisture product**
//!
//! ```text
//! (L_eff·μ)_crit = (1 + c_g)·m_p / ((1 − q_share)·g·Monod) = 2.439 · m_p / g   at N = 0.4
//! ```
//!
//! At the shipped defaults that is `0.305`, and the contract's own "average" reference cell
//! sits at `0.35` — 15 % above the threshold. It is why B0 measured average `P = 0.0977`
//! against a hand value of `0.50` (`design/7_Research/ecology-v1-implementation-2026-09-15.md`,
//! Run 3, B0): the average band is not a thin stand, it is a stand just barely above the point
//! where foliage cannot persist. `g` and `m_p` are the only two terms in that threshold that
//! this vector could otherwise not move, so leaving them out would mean searching how fast a
//! grazer eats a world that has nearly nothing to eat.
//!
//! `plant.wood_rate` and `plant.alpha` are the two names from the brief's list that were
//! dropped to make room; [`EXCLUDED`] says why.
//!
//! ## The fourteenth: the price of travel
//!
//! `organism.move_cost` was added after the calibration screen, for
//! `design/handoffs/ecology-v1-movement-opus-2026-09-16.md`. The screen moved no name that
//! couples an animal to a place and measured 0 foliage recovery events in 270 of 270 runs
//! while a prey body covered a quarter of the cube per window; this is the one knob already in
//! `WorldConfig` that makes leaving a cell cost anything. It is **appended**, so the first
//! thirteen indices are exactly where they were, and its default is the shipped value, so the
//! default vector still builds the world the screen ran — a check the movement campaign makes
//! explicitly against that screen's retained `final_state_hash`es.
//!
//! One consequence is deliberate and is not hidden: a result row recorded before this name
//! existed carries a thirteen-component `param_bits`, and [`from_bit_labels`] refuses it by
//! length rather than padding it. Those rows replay under the build that wrote them; this
//! build reproduces them by re-running the matrix and comparing state hashes, which is the
//! stronger check anyway.

use cubarium_core::WorldConfig;
use cubarium_core::hunter::FixedHunterProfile;

/// One searched scalar.
#[derive(Clone, Copy, Debug)]
pub struct ParamSpec {
    /// Dotted path: `<section>.<field>` on the world config.
    pub name: &'static str,
    /// Unit of the value, so a bound is readable without opening the contract.
    pub unit: &'static str,
    pub lo: f64,
    pub hi: f64,
    /// The value the shipped default carries, recorded so a run can say what it moved away from.
    pub default: f64,
    pub why: &'static str,
}

/// The fourteen searched parameters: four for primary production and foliage turnover, two for
/// the plant's reserve policy, three for animal intake and upkeep, three for maturation and
/// reproductive timing, one for recycling, and one — appended last — for the price of travel.
pub const PARAMS: &[ParamSpec] = &[
    // --- production and foliage turnover ------------------------------------------------
    ParamSpec {
        name: "producer.growth",
        unit: "m per m of foliage per second, at full light and no nutrient limit",
        lo: 0.004,
        hi: 0.020,
        default: 0.008,
        why: "`g`: primary production, and one of the two terms in the critical light \
              `(L·μ)_crit = 2.439·m_p/g` below which a cell holds no foliage at all. At the \
              default it is 0.305 and the contract's average band is 0.35, so most of the \
              cube sits just above the foliage extinction threshold. The upper bound moves \
              that threshold to 0.12, the lower one to 0.61 (almost the whole world bare)",
    },
    ParamSpec {
        name: "producer.mortality",
        unit: "fraction of foliage per second",
        lo: 0.0003,
        hi: 0.0020,
        default: 0.001,
        why: "`m_p`: leaf senescence, the dominant sink in the steady state `G = S` and the \
              other term in `(L·μ)_crit`. It is also what a grazer competes with: the \
              sustainable yield of a leaf-cap-limited stand is `r_p·W − m_p·P`, so halving \
              `m_p` roughly doubles what a stand can feed without being run down",
    },
    ParamSpec {
        name: "plant.foliage_rate",
        unit: "m of foliage per second per m of wood",
        lo: 0.001,
        hi: 0.010,
        default: 0.002,
        why: "`r_p`: the leaf-growth cap. In bright cells it, not income, sets the standing \
              crop, and it is the whole sustainable yield of such a stand. It is also the \
              reflush speed, so it is the direct lever on B6b's measured mismatch — prey \
              double in 150 s, a grazed stand needs 823 s to recover half its foliage",
    },
    ParamSpec {
        name: "plant.maintenance",
        unit: "m per second per m of wood",
        lo: 0.00005,
        hi: 0.00060,
        default: 0.0002,
        why: "`m_w`: the standing cost of structure. It sets the breakeven foliage a \
              defoliated stand has to reach before it can grow again, and, through `unpaid`, \
              the dieback rate that kills a stripped stand (B4a measured an e-fold of 5,110 s \
              at the default). Lowering it makes a stripped stand survivable; raising it \
              makes wood expensive and dieback fast",
    },
    // --- reserve policy -------------------------------------------------------------------
    ParamSpec {
        name: "plant.reserve_share",
        unit: "fraction of each tick's growth surplus",
        lo: 0.05,
        hi: 0.60,
        default: 0.2,
        why: "`q_share`: how much of a surplus is banked instead of grown. The bank is what \
              pays reflush after a graze and what makes a stand a §4.8 donor, but it is taken \
              off the top, so it also raises `(L·μ)_crit` and cost bright `W` 0.488 → 0.393 \
              over 30 min in run 1 against run 3 (finding R2-1). A real trade-off, searched",
    },
    ParamSpec {
        name: "plant.reflush_below",
        unit: "fraction of the structural foliage cap `α·W`",
        lo: 0.10,
        hi: 0.90,
        default: 0.25,
        why: "`p_reflush`: the emergency threshold below which reserve is spent on leaves. \
              Raising it lets a grazed stand start rebuilding much sooner and makes the \
              reserve, not the ceiling, the binding constraint (finding R2-4: `p_reflush·α = \
              q_cap` exactly at the defaults). This is the plant's only behavioural response \
              to being eaten",
    },
    // --- animal intake and upkeep ---------------------------------------------------------
    ParamSpec {
        name: "organism.mouth_rate",
        unit: "m per second for a unit adult at saturating food",
        lo: 0.010,
        hi: 0.080,
        default: 0.05,
        why: "the intake ceiling. §11: a unit grazer's bite at `P = 0.5` is 0.026 m/s against \
              a bright stand's sustainable 0.00064 m/s, ~40×, so one animal strips a cell in \
              seconds. The lower bound 0.010 is where a grazer standing on a full stand only \
              just clears its own resting upkeep (0.0053 against 0.0054 m/s), i.e. the floor \
              of animal viability",
    },
    ParamSpec {
        name: "organism.intake_half_saturation",
        unit: "m of edible stock",
        lo: 0.15,
        hi: 1.20,
        default: 0.45,
        why: "`K_P`: how thin a patch can still feed. Raising it makes a depleted cell stop \
              paying its occupant sooner, which is the only *refuge* mechanism the model has \
              — B1b measured stands stripped to `P ≈ 0.0004`, three orders below the default \
              `K_P`, so the type-II term is not currently protecting anything",
    },
    ParamSpec {
        name: "organism.maintenance",
        unit: "e per second per unit of structure",
        lo: 0.0020,
        hi: 0.0100,
        default: 0.005,
        why: "baseline upkeep: the income floor every animal has to clear, and 76 % of the \
              complete bill B6b measured (mandatory 2.33e-2 e/s against a motor 1.62e-3 e/s). \
              It converts directly into the foliage a body must find each second",
    },
    // --- maturation and reproductive timing ------------------------------------------------
    ParamSpec {
        name: "organism.growth_rate",
        unit: "m of structure per second",
        lo: 0.004,
        hi: 0.030,
        default: 0.01,
        why: "juvenile maturation speed. It gates whether offspring become breeding adults, \
              and with `bud_min_age_seconds` it sets the generation time that B6b measured at \
              a 150 s doubling against an 823 s plant recovery",
    },
    ParamSpec {
        name: "drives.bud_reserve",
        unit: "fraction of `reserve_max`",
        lo: 0.55,
        hi: 0.95,
        default: 0.7,
        why: "reproduction threshold: how fat a parent must be to conceive. Its range \
              straddles the core's own `child material exceeds the conception reserve` \
              constraint at 0.60, so the harness is required to reject and record part of its \
              own declared box rather than repair it",
    },
    ParamSpec {
        name: "drives.bud_min_age_seconds",
        unit: "seconds",
        lo: 60.0,
        hi: 900.0,
        default: 120.0,
        why: "prey generation time, the single most direct lever on the measured B6b failure: \
              the population doubles 5.5× faster than the stand it eats recovers. The upper \
              bound is 900 s, still well inside the 7,200 s lifespan, so a slowed lineage can \
              still reproduce more than once",
    },
    // --- recycling --------------------------------------------------------------------------
    ParamSpec {
        name: "detritus.decomposition",
        unit: "fraction of the litter stock per second",
        lo: 0.0005,
        hi: 0.0080,
        default: 0.002,
        why: "`k_d`: `D → N`, the rate at which the nutrient loop closes — and, because \
              litter is food, the rate at which a detritivore's larder is taken away by \
              microbes instead. It is the one knob that is simultaneously a plant income term \
              and an animal food term, which is why it is the recycling axis's representative",
    },
    // --- the price of travel ------------------------------------------------------------------
    ParamSpec {
        name: "organism.move_cost",
        unit: "e per unit of structure per pixel of distance travelled",
        lo: 0.00036,
        hi: 0.006,
        default: 0.00036,
        why: "the per-pixel coefficient of the motor bill `move_cost · S · (speed + k·r·|ω|) · \
              dt` (`crates/cubarium-core/src/motor.rs:354-408`), which charges translation by \
              distance and turning by swept distance. It is the only existing knob that makes \
              leaving a cell cost anything, and the ecology v1 screen moved **no** name that \
              couples an animal to a place, which is why it recorded 0 foliage recovery events \
              in 270 of 270 runs. The bounds are not invented: 0.00036 is what the world ships, \
              and 0.006 is the per-second coefficient the world charged *before* the pace \
              calibration divided it by the new `speed_max = 5.0` \
              (`crates/cubarium-core/src/config.rs:558-565`), so the top of the box restores \
              the per-pixel price of travel that existed before cruise speed rose",
    },
];

/// Candidates deliberately left out of this milestone's vector, and why. Recorded so the
/// exclusion is a decision rather than an oversight.
pub const EXCLUDED: &[(&str, &str)] = &[
    (
        "plant.wood_rate",
        "named by the calibration brief and dropped to keep the vector near a dozen. Wood \
         reaches the animal-facing ecology through exactly two couplings: the foliage cap \
         `P_cap = α·W`, which §11's arithmetic and B0's measurement both show does not bind \
         at the steady state, and the maintenance bill `m_w·W`, which `plant.maintenance` \
         already moves. Its one uncovered effect is how fast a re-established cell grows into \
         an alive stand and then a donor; the calibration reports recolonisations and \
         establishing cells so that effect is measured at the default rather than searched",
    ),
    (
        "plant.alpha",
        "named by the brief and dropped for the same reason: `P_cap = α·W` binds only on \
         painted stands and during reflush, and at §11's values `p_reflush·α = q_cap` exactly \
         (finding R2-4), so the reserve runs out before the ceiling is reached. Moving \
         `plant.reflush_below` moves the same ceiling with one fewer name",
    ),
    (
        "detritus.{wood_decomposition, carrion_decomposition}",
        "named by the brief as part of the recycling axis and held fixed. Dead wood only \
         exists after a stand dies and remains only after a body dies, so both stocks are \
         consequences of the coupling being calibrated rather than inputs to it; searching \
         them would let a candidate win by disposing of its own casualties faster. Both \
         stocks, and the intake taken from them, are reported per candidate instead",
    ),
    (
        "plant.{build, dieback, alive_min, donor_min, donor_reserve_floor, propagule_rate, \
         propagule_split, wood_max, reserve_cap, initial_wood}",
        "the structural and establishment block. `build` (`c_g`) and `dieback` (`κ`) are \
         accounting exponents the contract fixes at 0.2 and 1; the establishment five were \
         measured by A9 and B7 in the accepted milestone and changing them changes what \
         recolonisation means rather than how hard the ecology is; `wood_max`, `reserve_cap` \
         and `initial_wood` set the founder stock, which is held equal across candidates so a \
         candidate cannot win by being handed more plant material",
    ),
    (
        "organism.{capability_gate, capability_exponent}",
        "`θ` and `γ` (§6.1). The brief admits them only if the baseline shows generalist \
         dominance. The baseline measures population by guild — herbivore, detritivore and \
         generalist from `cap_foliage`/`cap_detrital` — so the condition is checked rather \
         than assumed; if a generalist sweep is what the default world does, these two are \
         the named follow-up and nothing else in this vector can substitute for them",
    ),
    (
        "organism.{assimilation_material, assimilation_energy, reserve_energy_density}, \
         plant.energy_density, fruit.energy_density",
        "energy-density and efficiency terms are cross-constrained by `WorldConfig::validate` \
         (`plant.energy_density ≥ reserve_energy_density · assimilation_material`, \
         `fruit.energy_density ≥ plant.energy_density`); searching them jointly would spend \
         most of the budget on rejections. Ecology v1 folded `producer.energy_density` into \
         `plant.energy_density`, the one `e_v` for foliage, wood, reserve and dead wood; the \
         removed key is named nowhere in this file",
    ),
    (
        "hunter.{capture_base, digest_rate, reproduce_min_age_seconds, \
         reproduce_interval_seconds, gestation_seconds}",
        "the apex profile is held at `FixedHunterProfile::lanternjaw_trial` in every arm, so \
         the zero-, one- and two-apex arms differ **only** in how many predators exist. A \
         searched profile would confound the predator's effect with the predator's design, \
         which is what the matched arms are for. The M1 search moved `capture_base` and \
         `digest_rate`; those scores were taken on the pre-ecology-v1 world and are not \
         evidence here",
    ),
    (
        "dormancy::{SUSTAIN_TICKS, STAGGER_TICKS, RECHECK_TICKS, PREY_RADIUS_PX, PREY_REQUIRED, \
         MAINTENANCE_PER_STRUCTURE_SECOND, EMERGENCE_RESERVE_FRACTION, EMERGENCE_ENERGY_FRACTION}, \
         encounter::{MATING_RADIUS_PX, COMBAT_RADIUS_PX, INJURY_ADULT_FRACTION}",
        "hardcoded `pub const` in the core; the brief holds them unsearched and asks for their \
         effect to be reported instead. Apex emergences, exhaustions, matings, births and \
         deaths are all in the component vector, so what they did is on the record",
    ),
    (
        "water.{rain_rate, evap, flow, algae_light}, habitat.*, weather.*",
        "these define the landscape and the seed's meaning, not the ecology running on it. \
         Changing them changes what a seed is, so candidates would no longer be compared on \
         one world. Held fixed at the shipped defaults, with default weather on",
    ),
    (
        "founders.{count, kinds}",
        "the initial animal stock is an accounted input held equal across candidates, not a \
         searched parameter. The four kinds are also the only source of guild variety at tick \
         0, so moving them would move the variety measurement itself",
    ),
    (
        "capacity.max_organisms",
        "a budget ceiling, not an ecological parameter; held at the default so a candidate \
         cannot win by being allowed more bodies",
    ),
];

/// The default vector, in [`PARAMS`] order.
pub fn defaults() -> Vec<f64> {
    PARAMS.iter().map(|p| p.default).collect()
}

/// `(lo, hi)` per parameter, in [`PARAMS`] order.
pub fn bounds() -> Vec<(f64, f64)> {
    PARAMS.iter().map(|p| (p.lo, p.hi)).collect()
}

/// The index of a searched name, or `None` if it is not searched. Used by the calibration's
/// declared candidates, so a typo in a candidate is an error rather than a silent default.
pub fn index_of(name: &str) -> Option<usize> {
    PARAMS.iter().position(|p| p.name == name)
}

/// Clamp every component into its bounds. Used after mutation and crossover, so the search
/// never proposes a value outside the declared box; it may still propose a value the core
/// itself rejects, which is the point.
pub fn clamp(values: &mut [f64]) {
    for (v, p) in values.iter_mut().zip(PARAMS) {
        if !v.is_finite() {
            *v = p.default;
        }
        *v = v.clamp(p.lo, p.hi);
    }
}

/// Write the vector into a config. The hunter profile is **not** written by any searched
/// parameter in ecology v1 (see [`EXCLUDED`]); it stays in the signature because
/// [`crate::evaluate`] builds one per evaluation and a later vector may move it again.
///
/// Errors only on a length mismatch or a non-finite component; a value the *core* considers
/// invalid is written through unchanged, so that `WorldConfig::validate` is the single place
/// that decides validity.
pub fn apply(
    values: &[f64],
    config: &mut WorldConfig,
    _profile: &mut FixedHunterProfile,
) -> Result<(), String> {
    if values.len() != PARAMS.len() {
        return Err(format!(
            "parameter vector has {} components, expected {}",
            values.len(),
            PARAMS.len()
        ));
    }
    for (v, p) in values.iter().zip(PARAMS) {
        if !v.is_finite() {
            return Err(format!("{} is not finite: {v}", p.name));
        }
        let v = *v;
        match p.name {
            "producer.growth" => config.producer.growth = v,
            "producer.mortality" => config.producer.mortality = v,
            "plant.foliage_rate" => config.plant.foliage_rate = v,
            "plant.maintenance" => config.plant.maintenance = v,
            "plant.reserve_share" => config.plant.reserve_share = v,
            "plant.reflush_below" => config.plant.reflush_below = v,
            "organism.mouth_rate" => config.organism.mouth_rate = v,
            "organism.intake_half_saturation" => config.organism.intake_half_saturation = v,
            "organism.maintenance" => config.organism.maintenance = v,
            "organism.growth_rate" => config.organism.growth_rate = v,
            "drives.bud_reserve" => config.drives.bud_reserve = v,
            "drives.bud_min_age_seconds" => config.drives.bud_min_age_seconds = v,
            "detritus.decomposition" => config.detritus.decomposition = v,
            "organism.move_cost" => config.organism.move_cost = v,
            other => return Err(format!("no writer for parameter {other}")),
        }
    }
    Ok(())
}

/// Read the vector back out, so a round trip can be tested and a replay can be checked.
pub fn read(config: &WorldConfig, _profile: &FixedHunterProfile) -> Vec<f64> {
    PARAMS
        .iter()
        .map(|p| match p.name {
            "producer.growth" => config.producer.growth,
            "producer.mortality" => config.producer.mortality,
            "plant.foliage_rate" => config.plant.foliage_rate,
            "plant.maintenance" => config.plant.maintenance,
            "plant.reserve_share" => config.plant.reserve_share,
            "plant.reflush_below" => config.plant.reflush_below,
            "organism.mouth_rate" => config.organism.mouth_rate,
            "organism.intake_half_saturation" => config.organism.intake_half_saturation,
            "organism.maintenance" => config.organism.maintenance,
            "organism.growth_rate" => config.organism.growth_rate,
            "drives.bud_reserve" => config.drives.bud_reserve,
            "drives.bud_min_age_seconds" => config.drives.bud_min_age_seconds,
            "detritus.decomposition" => config.detritus.decomposition,
            "organism.move_cost" => config.organism.move_cost,
            other => panic!("no reader for parameter {other}"),
        })
        .collect()
}

/// A stable fingerprint of a parameter vector, used to cache repeat evaluations of an elite.
/// Bit-exact on the `f64` pattern: two vectors share a fingerprint only if they are identical.
pub fn fingerprint(values: &[f64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in values {
        for b in v.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
    }
    h
}

/// The vector as `name -> value`, for a result row. **Readable, not authoritative**: a decimal
/// JSON round trip is not guaranteed to return the same `f64` bit for bit, and one ULP on
/// `producer.growth` is a different world. Use [`bit_labels`] for anything that has to reproduce.
pub fn labelled(values: &[f64]) -> serde_json::Map<String, serde_json::Value> {
    PARAMS
        .iter()
        .zip(values)
        .map(|(p, v)| (p.name.to_string(), serde_json::json!(v)))
        .collect()
}

/// The exact IEEE-754 bit pattern of each component, as 16 lowercase hex digits, in
/// [`PARAMS`] order. This is what a replay reads: it is the only encoding that is guaranteed
/// to hand the simulation back the number it was given.
pub fn bit_labels(values: &[f64]) -> Vec<String> {
    values
        .iter()
        .map(|v| format!("{:016x}", v.to_bits()))
        .collect()
}

/// Inverse of [`bit_labels`], with the length and the digits checked.
pub fn from_bit_labels(labels: &[String]) -> Result<Vec<f64>, String> {
    if labels.len() != PARAMS.len() {
        return Err(format!(
            "recorded vector has {} components, expected {}",
            labels.len(),
            PARAMS.len()
        ));
    }
    labels
        .iter()
        .zip(PARAMS)
        .map(|(text, p)| {
            u64::from_str_radix(text, 16)
                .map(f64::from_bits)
                .map_err(|e| format!("{}: {text:?} is not 16 hex digits: {e}", p.name))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every searched name must still exist on a schema 16 config, and writing the defaults
    /// back must leave a world the core will accept.**
    ///
    /// The list is prose, so this test is the thing that makes it checkable: it drives every
    /// entry through the real writers and the real reader, and then through
    /// `WorldConfig::validate`.
    #[test]
    fn every_searched_parameter_applies_to_a_schema_sixteen_config_and_validates() {
        let mut config = WorldConfig::default();
        let mut profile = FixedHunterProfile::lanternjaw_trial(&config);

        // The writers: a name with no writer is an error, not a silent skip.
        let values = defaults();
        apply(&values, &mut config, &mut profile).expect("every parameter has a writer");
        config
            .validate()
            .expect("writing the shipped defaults back leaves a valid schema 16 config");

        // The readers: the round trip is bit-exact, so no name reads a different field than
        // it writes.
        let back = read(&config, &profile);
        assert_eq!(back.len(), PARAMS.len());
        for (p, (wrote, got)) in PARAMS.iter().zip(values.iter().zip(&back)) {
            assert_eq!(
                wrote.to_bits(),
                got.to_bits(),
                "{}: wrote {wrote}, read back {got}",
                p.name
            );
        }

        // A moved value has to land somewhere the reader can see, so a name that writes into
        // a field nothing reads cannot pass either.
        for (k, p) in PARAMS.iter().enumerate() {
            let mut moved = defaults();
            moved[k] = 0.5 * (p.lo + p.hi);
            let mut config = WorldConfig::default();
            let mut profile = FixedHunterProfile::lanternjaw_trial(&config);
            apply(&moved, &mut config, &mut profile).unwrap_or_else(|e| panic!("{}: {e}", p.name));
            let back = read(&config, &profile);
            assert_eq!(
                back[k].to_bits(),
                moved[k].to_bits(),
                "{}: the midpoint did not survive the round trip",
                p.name
            );
        }
    }

    /// **Every default sits inside its own declared box**, so "the default vector" and "the
    /// centre of the search box" are never silently different worlds.
    #[test]
    fn defaults_lie_inside_their_bounds_and_match_the_shipped_config() {
        let shipped = WorldConfig::default();
        let profile = FixedHunterProfile::lanternjaw_trial(&shipped);
        let read_back = read(&shipped, &profile);
        for (k, p) in PARAMS.iter().enumerate() {
            assert!(
                p.lo < p.hi,
                "{}: lo {} is not below hi {}",
                p.name,
                p.lo,
                p.hi
            );
            assert!(
                p.lo <= p.default && p.default <= p.hi,
                "{}: default {} is outside [{}, {}]",
                p.name,
                p.default,
                p.lo,
                p.hi
            );
            assert_eq!(
                read_back[k].to_bits(),
                p.default.to_bits(),
                "{}: declared default {} is not what `WorldConfig::default` carries ({})",
                p.name,
                p.default,
                read_back[k]
            );
            assert!(!p.unit.is_empty(), "{} has no unit", p.name);
            assert!(!p.why.is_empty(), "{} has no rationale", p.name);
        }
    }

    /// The declared box is allowed to straddle the core's validity constraints, and one
    /// parameter is *required* to, so a rejected region is recorded rather than repaired.
    #[test]
    fn the_box_straddles_the_cores_own_conception_constraint() {
        let k = index_of("drives.bud_reserve").expect("bud_reserve is searched");
        let mut values = defaults();
        values[k] = PARAMS[k].lo;
        let mut config = WorldConfig::default();
        let mut profile = FixedHunterProfile::lanternjaw_trial(&config);
        apply(&values, &mut config, &mut profile).expect("the writer accepts an invalid value");
        assert!(
            config.validate().is_err(),
            "the low end of drives.bud_reserve must be refused by the core, not by the harness"
        );
    }

    /// The exclusion list is prose about real fields, so it must not name a key that no
    /// longer exists. `producer.energy_density` is the one ecology v1 removed.
    #[test]
    fn the_exclusion_list_names_no_removed_key() {
        for (names, why) in EXCLUDED {
            assert!(
                !names.contains("producer.energy_density"),
                "the exclusion list still advertises a key schema 16 does not have: {names}"
            );
            assert!(!why.is_empty(), "{names} has no reason");
        }
        // And the key that replaced it is named, so the list still covers the constraint.
        assert!(
            EXCLUDED
                .iter()
                .any(|(names, _)| names.contains("plant.energy_density")),
            "the one `e_v` must still be listed as excluded and why"
        );
    }

    /// No name may be both searched and excluded, and no name may be searched twice.
    #[test]
    fn searched_and_excluded_do_not_overlap() {
        let mut seen = std::collections::BTreeSet::new();
        for p in PARAMS {
            assert!(seen.insert(p.name), "{} is searched twice", p.name);
        }
        for p in PARAMS {
            for (names, _) in EXCLUDED {
                // `plant.{a, b}` style entries are checked by their exact dotted name, which
                // a brace list never contains verbatim.
                assert_ne!(*names, p.name, "{} is both searched and excluded", p.name);
            }
        }
    }
}
