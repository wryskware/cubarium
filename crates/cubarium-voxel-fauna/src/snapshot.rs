//! Save/load as postcard bytes behind a schema tag, the way
//! [`cubarium_voxel::snapshot`] does it. A different tag is refused; there is no
//! migration, ever.
//!
//! `always-fresh-never-migrate`: a new schema refuses old worlds and nothing is
//! synthesized or re-anchored. A saved animal layer whose [`crate::FaunaConfig`] no animal
//! can live under is refused too, for the reason [`crate::Fauna::new`] panics on one: it is
//! not a runtime condition.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::Fauna;

/// The animal layer's snapshot format. Bumped whenever a serialized field, the config
/// inside the snapshot, or what a saved field means changes. Postcard is not
/// self-describing, so an older world is **refused**, not migrated
/// (`always-fresh-never-migrate`): start a fresh world.
// 28: F1 (tests-first stubs) — `Pose::y_m`, `Mobility::{mode, perch}`, `Animal::last_bloom`,
// `FounderPhysiology::flight`, the bellwing lineage and the nectar food class.
// 29: F1 — `Perch::anchor`, `Mobility::falling`, `FounderState::lift`, `Flight::lift_speed_fraction`,
// the flight ledger lines and the bellwing's own physiology.
// 30: F3 (tests-first stubs) — the seedporter lineage, the fruit food class,
// `FounderPhysiology::{glider, gut_seeds}`.
// 31: F3 — the seedporter climbs, glides and carries gut seeds (`Animal::gut_seeds`,
// `Glider::roost_search_m`, the glider ledger lines).
// 32: R3 (tests-first stubs) — the chorister lineage, `FounderPhysiology::pack`,
// `Founding::Pack`, `Animal::pack`, `Fauna::packs`, the provisioning ledger lines.
// 33: R3 — the chorister lives: its physiology, the pack tactic's state on
// `PackRecord`, `Animal::{homing_ticks, yielding}`, `Escrow::young`, the pack and yield
// ledger lines, `PredationConfig::{carcass_yield_radius_m, threat_clear_m}`.
// 34: merge with main's crag fixes — `LifeConfig::wall_friction`,
// `FoundingConfig::clearance_margin_m` in the saved config.
// 35: roster mechanics — `LifeConfig::shallows_search_m`, `Glider::fruit_search_m` in
//     the saved config.
// 36: the minimal gut — `Animal::{stomach, eating, emergency, bout}`, the gut ledger
//     lines, `FounderPhysiology::{gut, emergency_clear_hunger}`, `Hunter` without its
//     satiated rest, `Rest` without `Satiated`, `FaunaConfig::parasite_take_per_day`,
//     `LifeConfig::ecological_day_s`; per-day upkeep and the new lifespans.
// 37: springturf spread — `FounderPhysiology::burr_coat`.
// 38: scenario maps — `FoundingConfig::reference_land_m2` in the saved config.
// 39: the detritus web — `Activity::DarkActive`, `Reproduction::{clutch_max,
//     nest_radius_m, nest_crowd_max, nest_retry_s}`, `ReproductionState::{native,
//     retry_ticks}`, `FaunaConfig::lineage_body_ceiling`, `TeacherConfig::{
//     forage_stay_forward, forage_turn_back_samples}`, the readout ledger lines.
// 40: the capgnawer package — `EggSite::Fungus`, `Reproduction::{clutch_hold_max_s,
//     egg_fungus_radius_m}`, `ReproductionState::clutch_wait_ticks`,
//     `CueConfig::fungal_half_life_s`.
// 41: generation time (D1b) — `Gut::mass_exponent`, `Reproduction::brood_share`,
//     `ReproductionState::brood`.
// 42: the lanternjaw's patrol (D2) — `FaunaConfig::hunt`, `Animal::hunt`, the patrol's
//     ledger lines, the lanternjaw's clutch from surplus and smaller reserve cap.
// 43: the lanternjaw's switching (D2b) — `HuntConfig::{share_switching, switch_exponent,
//     attack_prior_s, strike_chosen_only}`, the declined-contact and unchosen-strike ledger
//     lines.
// 44: HUNTER-MEAL — `HuntConfig::{meal_first, carcass_scan_m, reachable_only,
//     route_refresh_s, success_memory_s, success_boost}`, `HuntMemory::productive`, the meal
//     and route ledger lines.
// 45: VEILHOUND-SEARCH — `FounderPhysiology::senses` (`SenseTable`), `PackRecord::drawn`,
//     the senses' ledger lines.
// 46: SENSES-CORE — `SenseTable`'s new fields (night and still sight, hearing's floor,
//     tremor, smell, live, scent channel), `FaunaConfig::{perception, scent}`, the percept
//     records (`Fauna::percepts`), `PackRecord::drawn_scent`, `ChaseEnd::Lost`, the funnel
//     and the perception ledger lines; `pack_*_by_sense` widened to four modalities.
// 47: S2 GROUND RESPONSES — `Percept::vigil` (`Vigil`), `Track::heading`, `SenseTable::
//     {feeding_sight_fraction, kin_tracks}`, `FounderPhysiology::vigilance`, `Pack::{stalk_margin,
//     commit_m, stalk_freeze}`, `PredationConfig::{awareness_sensed, awareness_unaware,
//     aware_within_s}`, `HuntConfig::patch_scent_weight`, the `vigil` ledger lines.
// 48: AMPHIBIOUS SNAIL — `FounderPhysiology::amphibious` (`Amphibious`: the water rule, the
//     drying clock, the thirst return, retraction and the roam reflex), `Mobility::{dryness,
//     ashore, retract_ticks, thirsty}`, `Departure::Desiccation`, `Fauna::last_rain_tick`,
//     the ledger's `amphibious_ticks`, `retracts` and `ashore_assimilated`.
// 49: HUNKER — `FounderPhysiology::hunker` (`Hunker`), `Animal::hunkered`, `Vigil::{flee_id,
//     flee_tick, hunkered, hunker_close, pry, opened_until}`, the `vigil` ledger's hunker lines.
// 50: GAIT — `FounderPhysiology::sprint` (`Sprint`), `FounderState::{sprint_spent, winded}`,
//     `Glider::glide_speed_fraction` (replaces `glide_body_lengths_per_s`), the ledger's
//     `sprint_ticks_by_founder` and `winded_by_founder`; the Veilhound's and the seedporter's
//     cruise and motor defaults.
// 51: PACK-TUNE — `Pack::{select_kin_m, select_kin_factor, select_kin_max,
//     select_aware_factor, select_wound_gain, select_airborne_factor, select_body_exp,
//     select_judge_m, select_min_prospect, break_off_below, shun_s}`, `PackRecord::shunned`,
//     `ChaseEnd::BrokeOff`, the ledger's `pack_break_offs` and `pack_declined`; the hunker's
//     `pry_per_member_s` default and the resistance's growth with the body.
// 52: S3 LIGHT AND FLIGHT with S3b WHICH LIGHT PAYS — `SenseTable::{light_m, light_min, uv,
//     loom_m}` (the bellwing's and the capgnawer's tables live), `ScentChannel::Nectar` and
//     `ScentConfig::nectar_per_s`, `FaunaConfig::light` (with `learn, uv_innate, cyan_innate,
//     learn_rate, forget_s, expect_min, loom_tau_s`), the percept record's light reading,
//     failed-visit marks and visit, `Percept::{expect, expect_tick, last_sip}`, the lanternjaw
//     `DarkActive`, the light ledger lines (`light_budget`, `stalled_visits`, `sipped_visits`,
//     `naive_kills`).
// 53: CHURN — `Pack::{chase_progress_m, switch_margin}`, `PackRecord::{chase_progress_tick,
//     chase_best_m, chase_work}`, `ChaseEnd::Switched`, the ledger's `pack_switches`; a
//     chase's give-up reads time without progress, not a chase clock. CHURN2 (same 53, the
//     branch unmerged): `Pack::switch_stall_s`; a switch only after that long stalled.
//     STRIKE (same 53, the branch unmerged): `Pack::{close_in_m, stand_off_reach_fraction}`,
//     the ledger's `pack_close_in_samples, pack_step_offs, pack_pivots, pack_out_of_reach`.
// 54: LIFE-HISTORY — `Reproduction::{litters_per_year, breeding_phase, breeding_window,
//     gestation_upkeep_factor}`, `Pack::{breed_reserve_fraction, litter_by_condition}`,
//     `ReproductionState::season` and `PackRecord::season` (`Season`), the ledger's
//     `pups_independent` and `season_waits`; the Veilhound's season, gestation and rearing.
// 55: STRIKE-2 — `Pack::herd_shun_m`; a member out of its target's jaw band steps to a level
//     that meets it, and a pack that drops an armoured target leaves its armoured herd-mates.
// 56: PIT and PIT-2 — `Pack::{pit_search_m, route_swing_voxels, whelp_at_den}`; a pup or a
//     Veilhound going home follows the route, a body off its graph walks back on, dens on
//     founding faces, route walks land on the route at a bend.
// 57: BELLWING — `LightConfig::{lure_uv, lure_scent}` (the lure mimics a flower: UV and the
//     nectar scent); the bellwing's r-strategist reproduction defaults.
// 58: EDGE and EDGE-2 — `TeacherConfig::{press_replan_s, press_shun_s, route_refresh_s,
//     route_detour, way_on_m}`; reflex walks take reachable stands by the route, a pressed walk
//     re-plans, a flight turns along the strip's end, a body caught in solid steps out.
// 59: FINDINGS-FIX-2 — `Strike::lure` (a lure kill is booked by the lure draw at the lunge).
// 60: DROP — `FounderPhysiology::drop` (`DropRule`), `Departure::Fell`, the ledger's
//     `drops_by_founder`, `drop_m_by_founder` and `drop_wound_by_founder`; a trapped body jumps
//     down past its step at a wound.
// 61: SNAIL-LAND — `EggSite::Shore`, `Amphibious::{egg_water_max_m, egg_climb_max_m,
//     egg_moisture_min, egg_drown_s, egg_dry_moisture, egg_dry_s}`, `Clutch::{wet_ticks,
//     dry_ticks}`, the ledger's `eggs_drowned_by_founder` and `eggs_dried_by_founder`.
//     SNAIL-LAND-2 (same 61) — `Amphibious::{egg_search_m, graze_give_up, graze_worth, graze_search_m}`.
// 62: GLOWCAP — living mycelium intake by founder, included in CapTissue.
pub const SCHEMA: u32 = 62;

#[derive(Serialize, Deserialize)]
struct Envelope {
    schema: u32,
    fauna: Fauna,
}

pub fn encode(fauna: &Fauna) -> Vec<u8> {
    postcard::to_stdvec(&Envelope {
        schema: SCHEMA,
        fauna: fauna.clone(),
    })
    .expect("a Fauna always serializes")
}

pub fn decode(bytes: &[u8]) -> anyhow::Result<Fauna> {
    let tag: u32 = postcard::from_bytes(bytes).context("not a voxel fauna snapshot")?;
    if tag != SCHEMA {
        bail!("voxel fauna snapshot schema {tag} is not {SCHEMA}; start a fresh world");
    }
    let env: Envelope = postcard::from_bytes(bytes).context("corrupt voxel fauna snapshot")?;
    if let Err(e) = env.fauna.config().validate() {
        bail!("invalid voxel fauna snapshot — {e}");
    }
    if let Err(e) = validate(&env.fauna) {
        bail!("invalid voxel fauna snapshot — {e}");
    }
    Ok(env.fauna)
}

/// **Riders** (schema 22): every rider's host is present, is not of the rider's own
/// lineage, does not itself ride, and carries no more than `parasite_max_per_host`
/// riders — the conditions the hatching rule establishes and the drain and the carry
/// read.
fn riders(fauna: &Fauna) -> Result<(), String> {
    let animals = &fauna.view().animals;
    let most = fauna.config().parasite_max_per_host as usize;
    for a in animals.iter() {
        let Some(hold) = a.attached else { continue };
        let host = animals
            .binary_search_by_key(&hold.host, |h| h.id)
            .ok()
            .map(|j| &animals[j])
            .ok_or_else(|| {
                format!(
                    "animal #{} rides host #{}, which is not here",
                    a.id, hold.host
                )
            })?;
        if host.founder == a.founder {
            return Err(format!(
                "animal #{} rides host #{}, a body of its own lineage",
                a.id, host.id
            ));
        }
        if host.attached.is_some() {
            return Err(format!(
                "animal #{} rides host #{}, itself a rider",
                a.id, host.id
            ));
        }
        let on = animals.iter().filter(|r| r.attached == Some(hold)).count();
        if on > most {
            return Err(format!(
                "host #{} carries {on} riders, more than parasite_max_per_host {most}",
                host.id
            ));
        }
    }
    Ok(())
}

/// What a loaded layer has to satisfy beyond its schema tag and its config: the
/// **state**, and not only the configuration.
///
/// Three things, each of which an invariant of this layer depends on rather than a taste:
/// every stock is a finite nonnegative number, because a `NaN` body is a `NaN` in the
/// ledger within one step and a negative one is matter that does not exist; the ids are
/// **sorted and unique**, because every pass over `animals` is a binary search over that
/// order; and [`crate::FaunaLedger::births`] — the counter the next id comes from — is
/// **above every id present**, because an id is never reused and `Fauna::insert` treats a
/// collision as unreachable. A phase-one body also has to carry a finite [`crate::Pose`],
/// because the founder's movement and sensing read it and a `NaN` position is a `NaN` in
/// the world within a tick. And a founder's held actions are bounded actions with finite
/// feedback — the loader refuses anything that would drive a body outside the manifest's
/// caps or feed the observation a non-finite channel.
///
/// A [`crate::Clutch`] is held to the same three conditions as a body — finite
/// nonnegative currencies, at least one egg, and a laying tick that is not in the
/// layer's future — because it is a paid package the stored totals count and an
/// incubation counted from a future tick would never hatch.
///
/// Postcard bytes are not the only way in: a hand-built or hand-edited snapshot is exactly
/// what this is for, and a round trip of a live layer cannot fail it.
fn validate(fauna: &Fauna) -> Result<(), String> {
    let v = fauna.view();
    riders(fauna)?;
    let mut last: Option<u64> = None;
    for a in v.animals {
        for (field, value) in [
            ("body", a.body),
            ("reserve", a.reserve),
            ("mineral", a.mineral),
            ("energy", a.energy),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!(
                    "animal #{}'s {field} is {value}, not a finite nonnegative stock",
                    a.id
                ));
            }
        }
        // The minimal gut (repair 3, Astra's finding 9): every stomach pool's three
        // currencies are finite nonnegative stocks too.
        for (k, p) in a.stomach.pools.iter().enumerate() {
            for (field, value) in [("organic", p.organic), ("mineral", p.mineral), ("energy", p.energy)] {
                if !value.is_finite() || value < 0.0 {
                    return Err(format!(
                        "animal #{}'s stomach pool {k} {field} is {value}, not a finite nonnegative stock",
                        a.id
                    ));
                }
            }
        }
        if !a.pose.is_finite() {
            return Err(format!("animal #{}'s pose is not finite", a.id));
        }
        if let Some(f) = a.founder {
            let held = a.founder_state.held;
            // GAIT: a rush or a ground dash holds a forward up to its lineage's sprint.
            let fp = fauna.config().founder(f);
            let top = fp.sprint.effort(fp.cruise_body_lengths_per_s).max(1.0);
            for (field, value, most) in [("forward", held.forward, top), ("feed", held.feed, 1.0)] {
                if !value.is_finite() || !(0.0..=most).contains(&value) {
                    return Err(format!(
                        "animal #{}'s held {field} is {value}, not a bounded action",
                        a.id
                    ));
                }
            }
            if !held.turn.is_finite() || !(-1.0..=1.0).contains(&held.turn) {
                return Err(format!(
                    "animal #{}'s held turn is {}, not a bounded action",
                    a.id, held.turn
                ));
            }
            let fb = a.founder_state.feedback;
            for (field, value) in [
                ("intake", fb.intake),
                ("structural_loss", fb.structural_loss),
                ("attempted equivalent displacement", fb.attempted_equivalent),
                ("delivered equivalent displacement", fb.delivered_equivalent),
                ("delivered forward", fb.delivered_forward),
                ("motor respiration", fb.motor_respiration),
            ] {
                if !value.is_finite() || value < 0.0 {
                    return Err(format!(
                        "animal #{}'s {field} feedback is {value}, not finite nonnegative",
                        a.id
                    ));
                }
            }
            if !fb.delivered_turn.is_finite() {
                return Err(format!(
                    "animal #{}'s delivered turn feedback is not finite",
                    a.id
                ));
            }
        }
        // Predation (schema 20): the wound stock, the body's speed, and a strike's phase.
        if !a.wound.is_finite() || !(0.0..=1.0).contains(&a.wound) {
            return Err(format!(
                "animal #{}'s wound is {}, not a finite stock within 0..=1",
                a.id, a.wound
            ));
        }
        if !a.speed_m_s.is_finite() || a.speed_m_s < 0.0 {
            return Err(format!(
                "animal #{}'s speed is {}, not finite nonnegative",
                a.id, a.speed_m_s
            ));
        }
        if let Some(st) = a.strike {
            let hunter = a
                .founder
                .and_then(|f| fauna.config().founder(f).hunter)
                .ok_or_else(|| format!("animal #{} is striking and is no hunter", a.id))?;
            let most = match st.phase {
                crate::StrikePhase::Windup => crate::Hunter::ticks(hunter.windup_s),
                crate::StrikePhase::Lunge => crate::Hunter::ticks(hunter.lunge_s),
                crate::StrikePhase::Recovery => crate::Hunter::ticks(hunter.recovery_s),
            };
            if !(st.from_x.is_finite() && st.from_z.is_finite())
                || st.ticks_left == 0
                || st.ticks_left > most
            {
                return Err(format!(
                    "animal #{}'s strike is malformed: {:?} with {} ticks left (at most {most})",
                    a.id, st.phase, st.ticks_left
                ));
            }
        }
        let r = a.reproduction;
        if let Some(e) = r.escrow {
            for (field, value) in [
                ("escrowed organic", e.organic),
                ("escrowed mineral", e.mineral),
                ("escrowed energy", e.energy),
            ] {
                if !value.is_finite() || value < 0.0 {
                    return Err(format!(
                        "animal #{}'s {field} is {value}, not a finite nonnegative stock",
                        a.id
                    ));
                }
            }
        }
        if let Some(prev) = last {
            if a.id <= prev {
                return Err(format!(
                    "animal ids are not sorted and unique: #{} comes after #{prev}",
                    a.id
                ));
            }
        }
        last = Some(a.id);
    }
    if let Some(max) = last {
        if v.ledger.births <= max {
            return Err(format!(
                "the next id is {} and animal #{max} already holds it: an id is never reused",
                v.ledger.births
            ));
        }
    }
    // A clutch is a paid package the layer's stored totals count, so the same three
    // conditions apply to it as to a body: finite nonnegative currencies, and at least
    // one egg — a clutch of nothing is matter with nowhere to go.
    for c in v.clutches {
        if c.count == 0 {
            return Err(format!(
                "the clutch at {:?} holds no eggs: a clutch of nothing cannot hatch",
                c.site
            ));
        }
        for (field, value) in [
            ("organic", c.organic),
            ("mineral", c.mineral),
            ("energy", c.energy),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!(
                    "the clutch at {:?} has {field} {value}, not a finite nonnegative stock",
                    c.site
                ));
            }
        }
        if let Some(d) = c.dormancy {
            let heard_late = d.heard.iter().any(|&(id, t)| id != 0 && t > v.tick);
            if c.count != 1 || d.threshold == 0 || d.viable_until_tick < c.laid_tick || heard_late
            {
                return Err(format!(
                    "the dormant egg at {:?} is malformed: count {}, threshold {}, viable \
                     until {} (laid {}), a footfall heard after tick {}",
                    c.site, c.count, d.threshold, d.viable_until_tick, c.laid_tick, v.tick
                ));
            }
        }
        if c.laid_tick > v.tick {
            return Err(format!(
                "the clutch at {:?} was laid on tick {} and the layer is on tick {}: a \
                 clutch cannot be laid in the future",
                c.site, c.laid_tick, v.tick
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Animal, FaunaConfig, FaunaLedger, Site, Species, State};

    /// Predation's state is checked too: a NaN wound is refused, and a wound in range
    /// loads.
    #[test]
    fn a_nan_wound_is_refused() {
        let mut bad = animal(0, 0.05);
        bad.wound = f64::NAN;
        let err = decode(&encode(&layer(vec![bad], 1))).expect_err("a NaN wound");
        assert!(format!("{err:#}").contains("wound"), "{err:#}");
        let mut ok = animal(0, 0.05);
        ok.wound = 0.4;
        decode(&encode(&layer(vec![ok], 1))).expect("a wound in range loads");
    }

    /// A layer built by hand, which is the only way to write a snapshot this module has to
    /// refuse: a live layer's own bytes cannot fail these checks.
    fn layer(animals: Vec<Animal>, births: u64) -> Fauna {
        let mut fauna = Fauna::new(FaunaConfig::default());
        fauna.ledger = FaunaLedger {
            births,
            ..FaunaLedger::default()
        };
        fauna.animals = animals;
        fauna
    }

    fn animal(id: u64, body: f64) -> Animal {
        Animal {
            id,
            species: Species::Frondgrazer,
            site: Site { x: 1, y: 2, z: 0 },
            pose: crate::Pose::at_site(Site { x: 1, y: 2, z: 0 }, 1.0),
            founder: None,
            founder_state: crate::FounderState::default(),
            reproduction: crate::ReproductionState::default(),
            body,
            reserve: 0.5 * body,
            mineral: 0.05 * body,
            energy: 2.0 * body,
            age_ticks: 0,
            lifespan_ticks: 0,
            gut: cubarium_voxel_flora::Taken::default(),
            state: State::Resting,
            mobility: crate::Mobility::default(),
            wound: 0.0,
            strike: None,
            rest: None,
            still_ticks: 0,
            speed_m_s: 0.0,
            alert_ticks: 0,
            attached: None,
            last_bloom: None,
            gut_seeds: crate::GutSeedLots::default(),
            pack: None,
            homing_ticks: 0,
            yielding: false,
            hunkered: false,
            stomach: crate::Stomach::default(),
            eating: false,
            emergency: false,
            bout: None,
            hunt: crate::HuntMemory::default(),
        }
    }

    /// **Schema 6 carries the founder bodies and refuses an older world.**
    ///
    /// The bodies: lineage marker, pose, held actions and prior-interval feedback all
    /// come back as they went in. The lineage's `policy_driven` flag comes back too,
    /// which is the whole reason the schema moved — a loader that could not tell a
    /// trained lineage from a heuristic one would put the heuristic back in silence.
    /// The controllers themselves do **not** come back: a mind is session state, and a
    /// loaded layer's founders rest until a driver installs controllers again.
    ///
    /// The refusal is checked by rewriting the leading schema tag, which is one postcard
    /// varint byte. No hash is pinned and no old bytes are kept in the tree.
    #[test]
    fn the_current_schema_round_trips_a_founder_world_and_refuses_an_older_one() {
        let mut body = animal(0, 0.02);
        body.founder = Some(crate::Founder::Browser);
        body.founder_state.held = crate::Actions {
            forward: 0.75,
            turn: -0.5,
            feed: 1.0,
        };
        body.founder_state.feedback.intake = 1e-4;
        body.pose = crate::Pose {
            heading_rad: 1.25,
            ..body.pose
        };

        let mut fauna = layer(vec![body], 1);
        fauna.set_policy_driven(crate::Founder::Browser, true);
        fauna.set_policy_digest(crate::Founder::Browser, 0xdead_beef_cafe_1234);
        fauna.ledger.bites_by_founder[crate::Founder::Browser.index()] = 3;
        fauna.ledger.assimilated_by_founder[crate::Founder::Browser.index()] = 2.5e-4;
        // A mind installed before the save is not part of the save.
        assert!(fauna.set_controller(
            0,
            Box::new(crate::Scripted::new(vec![body.founder_state.held]))
        ));

        let bytes = encode(&fauna);
        let mut back = decode(&bytes).expect("a founder world round-trips");
        assert_eq!(back.view().animals, fauna.view().animals);
        assert_eq!(back.view().ledger, fauna.view().ledger);
        assert!(back.policy_driven(crate::Founder::Browser));
        assert!(!back.policy_driven(crate::Founder::Blind));
        assert_eq!(
            back.policy_digest(crate::Founder::Browser),
            0xdead_beef_cafe_1234
        );
        assert_eq!(back.policy_digest(crate::Founder::Blind), 0);
        assert!(
            back.take_controller(0).is_none(),
            "a loaded layer's founders rest until a driver installs controllers again"
        );

        let mut older = bytes.clone();
        assert_eq!(older[0], SCHEMA as u8, "the tag is the leading varint byte");
        older[0] = SCHEMA as u8 - 1;
        let err = format!("{:#}", decode(&older).expect_err("an older world"));
        assert!(err.contains("start a fresh world"), "{err}");
        assert!(err.contains(&format!("is not {SCHEMA}")), "{err}");
    }

    /// A founder's held actions are bounded actions and its feedback is finite: the
    /// loader refuses a hand-built layer that would move a body outside the manifest's
    /// caps or feed the observation a non-finite channel.
    #[test]
    fn the_loader_refuses_a_founder_state_it_could_not_have_produced() {
        let mut held = animal(0, 0.02);
        held.founder = Some(crate::Founder::Blind);
        held.founder_state.held.turn = 7.0;
        let err = format!(
            "{:#}",
            decode(&encode(&layer(vec![held], 1))).expect_err("held turn 7")
        );
        assert!(err.contains("held turn"), "{err}");

        let mut feed = animal(0, 0.02);
        feed.founder = Some(crate::Founder::Blind);
        feed.founder_state.held.feed = -0.5;
        let err = format!(
            "{:#}",
            decode(&encode(&layer(vec![feed], 1))).expect_err("held feed -0.5")
        );
        assert!(err.contains("held feed"), "{err}");

        let mut nan = animal(0, 0.02);
        nan.founder = Some(crate::Founder::Blind);
        nan.founder_state.feedback.structural_loss = f64::NAN;
        let err = format!(
            "{:#}",
            decode(&encode(&layer(vec![nan], 1))).expect_err("a NaN channel")
        );
        assert!(err.contains("structural_loss"), "{err}");

        // And a body with no lineage, whose founder state is all default, never trips it.
        assert!(decode(&encode(&layer(vec![animal(0, 0.02)], 1))).is_ok());
    }

    /// **R9.6: the loader validates the state.** Two well-formed animals round-trip; a
    /// `NaN` or negative stock, an unsorted or repeated id, and a next-id counter that
    /// would reuse an id are each refused with the reason named.
    #[test]
    fn the_loader_refuses_a_snapshot_whose_state_cannot_be_true() {
        let good = layer(vec![animal(0, 0.02), animal(3, 0.04)], 4);
        let back = decode(&encode(&good)).expect("a well-formed layer");
        assert_eq!(back.view().animals, good.view().animals);

        let cases: [(&str, Fauna); 6] = [
            ("body", layer(vec![animal(0, f64::NAN)], 1)),
            ("reserve", {
                let mut a = animal(0, 0.02);
                a.reserve = -1e-9;
                layer(vec![a], 1)
            }),
            ("energy", {
                let mut a = animal(0, 0.02);
                a.energy = f64::INFINITY;
                layer(vec![a], 1)
            }),
            ("pose", {
                let mut a = animal(0, 0.02);
                a.pose = crate::Pose {
                    x: f64::NAN,
                    ..a.pose
                };
                layer(vec![a], 1)
            }),
            (
                "sorted and unique",
                layer(vec![animal(3, 0.02), animal(1, 0.02)], 4),
            ),
            (
                "never reused",
                layer(vec![animal(0, 0.02), animal(3, 0.02)], 3),
            ),
        ];
        for (what, fauna) in cases {
            let err = format!(
                "{:#}",
                decode(&encode(&fauna)).expect_err("a refusable layer")
            );
            assert!(err.contains(what), "{what}: {err}");
        }

        // A duplicate id is the same refusal as an unsorted one, and an empty layer with a
        // counter of its own is fine: nothing holds an id.
        let err = format!(
            "{:#}",
            decode(&encode(&layer(vec![animal(2, 0.02), animal(2, 0.03)], 3)))
                .expect_err("two animals cannot share an id")
        );
        assert!(err.contains("sorted and unique"), "{err}");
        assert!(decode(&encode(&layer(Vec::new(), 7))).is_ok());
    }
}
