//! **Ground responses** (S2 of the senses pass, `design/7_Research/senses-review-2026-10-06.md`
//! §1, §2, §4 and §5 S2, with Wrysk's choices of 2026-10-06): what a live prey lineage does
//! with its percept record ([`crate::Percept`]). Each response reads only the record and keeps
//! its own state in the record's [`Vigil`]; its numbers are the lineage's [`Vigilance`]
//! (`fauna.founders.<lineage>.vigilance.*`), every one a plausible start, untuned.
//!
//! - **Herd** (the frondgrazer, the loftstrider):
//!   - a **scan pause** while cropping: every `scan_every_s` of cropping it stands with its
//!     head up for `scan_s`. Between scans its head is down and it sees only within
//!     [`crate::SenseTable::feeding_sight_fraction`] of its range: its blind window;
//!   - **orients** to an approaching threat (seen moving or near, heard, or felt) it has not
//!     faced yet: stands and faces it for `orient_s`, then grazes on while it stays away
//!     (habituated to that one body);
//!   - **flees with hysteresis**: it starts when at least `group_min` threats (one, for a
//!     juvenile) are within `flee_start_m` — seen moving or near, heard, or felt — and runs
//!     straight away from the nearest; it stops only once it is beyond `flee_stop_m` of where
//!     it last knew a threat and none has come within that for `calm_s`;
//!   - **herd alarm by hearing**: a running body is loud (its emission times `run_noise`
//!     while it flees or bolts), so a kin it hears whose emission (loudness times distance)
//!     is at least `alarm_emission_factor` times that kin's walking emission is bolting, and
//!     it bolts too: along that kin's heading for `alarm_bolt_s`, then deaf to alarms for
//!     `alarm_refractory_s`, so a wave crosses the herd once and does not echo.
//! - **Arboreal** (the seedporter): a threat within `flee_start_m` sends it to the nearest
//!   stand it can climb within `refuge_search_m` (walking into the stem climbs it), or
//!   straight away with none; in the crown it holds still until calm (the same hysteresis).
//! - **Freeze** (the littershredder): footfall at least `freeze_tremor` freezes it for
//!   `freeze_s` (no step, no bite); at least `withdraw_tremor` it also turns away in place. It
//!   never flees: it stays on its litter.
//!
//! **FEAR** (Wrysk, 2026-10-06: "Should seedporters ignore a resting or sleeping Veilhound,
//! the way gazelles graze near resting lions?" — "sure."). A lineage with `watch_calm` on
//! **watches** a hunter it sees asleep, or awake and resting ([`crate::perception::posture_of`]:
//! still, no strike, not feeding) and not facing it (its heading more than `calm_facing_deg`
//! off the bearing to the prey): that hunter is not a threat to its flight, its hunker or its
//! orientation. Inside `calm_margin_m` of one it **walks off** (its walk, no sprint, no refuge
//! run, no feeding; [`Vigil::calm_walk`]) until it is outside. It fears the hunter fully again
//! as soon as it wakes, stands, faces it, or approaches (S2's "pressing": seen moving, heard,
//! felt). Wild models: Thomson's gazelles graze in view of resting cheetahs and lions and keep
//! their distance, and monitor rather than flee a predator that is not hunting (Fitzgibbon
//! 1990, Anim. Behav. 40:837; Caro 2005, *Antipredator Defenses in Birds and Mammals*, ch. 4–5
//! on predator monitoring and flight-initiation distance).
//!
//! **FEAR-2** (FEAR's check: seedporters still in flight 75 % of their awake time). Three
//! holds kept a flight going after the prey was safe, and the fix ends each by behaviour:
//! - **the refuge**: a seedporter's flight stayed on through its whole perch (FEAR-2's probe:
//!   0.48–1.0 of its flight ticks perched, mostly not eating). A flight now **ends on reaching
//!   a perch**; the **refuge watch** ([`Vigil::refuge`]) keeps it up there — no step down, no
//!   glide, turning and feeding as it asks — while a hunter it does not watch is within
//!   `flee_start_m` (then `flee_stop_m`), and `lost_calm_s` (at least `calm_s`) after the last;
//! - **the lost hunter**: a flight held while its hunter was out of every sense and the body
//!   within `flee_stop_m` of where it last knew it (a perched or stuck body never gets beyond;
//!   0.83–1.0 of the probe's flight ticks). No hunter known within `flee_stop_m` for
//!   `lost_calm_s` ends it where it is;
//! - **the idle hunter**: FEAR's watch read only a hunter in [`State::Resting`]; one standing
//!   idle (awake and still in another state) stayed a threat at any heading. Still and facing
//!   away, it is watched in any awake state ([`crate::perception::posture_of`]); still and
//!   facing the prey (a stalk's freeze) it stays a threat; awake within the predation table's
//!   `still_threat_near_m` (in contact: a hunter working at a hunkered grazer) it is a threat
//!   whichever way it faces.
//!
//! The awareness the strike's dodge reads ([`awareness`], D5) and the Veilhound's stalk
//! ([`stalk`]) are here too: both read the prey's record.
//!
//! **HUNKER** (Wrysk, 2026-10-06: the frondgrazer defends with armour, not speed; its body is
//! a low six-legged animal under a rigid three-plate saddle shield). A lineage whose
//! [`Hunker`] is on (`fauna.founders.<lineage>.hunker.*`) answers a close hunter by
//! **hunkering** instead of fleeing:
//! - **the hunker**: an approaching threat within `enter_m` folds its neck into the shield's
//!   front notch and clamps it down: it stands, and it cannot eat. It comes out once no
//!   approaching threat has been within `leave_m` for the calm, `calm_s` × its condition
//!   (reserve over cap) to `hunger_calm_exp`: a hungrier body comes out sooner. Below
//!   `min_condition` it cannot afford to hunker at all;
//! - **bunching**: the herd alarm (a kin heard running) moves it toward the kin it knows,
//!   not along the runner's heading, and a threat within `flee_start_m` (beyond `enter_m`)
//!   sends a body with kin farther than `ring_m` running to them, loud, which alarms the rest.
//!   A body with no kin known hunkers where it stands;
//! - **the shield** ([`guard`]): a strike on a hunkered body glances off with the chance
//!   [`Hunker::shield_of`]: `shield` × (body over the adult body)^`shield_growth_exp` ×
//!   (1 − `wound_opens` × wound) — the young are soft, the wounded split open. It neither
//!   dodges nor runs;
//! - **the pack's work** ([`pry`]): each awake hunter of it in contact (centres within half
//!   their summed lengths plus `pry_reach_m`) pries at `pry_per_member_s`; the body resists at
//!   `resist_per_s` × (its body over the adult's)^`shield_growth_exp` (PACK-TUNE: the young
//!   are soft to prying too) × (1 + `ring_resist_per_kin` per hunkered kin within `ring_m`, at
//!   most `ring_kin_max`). At progress 1 it is turned over: **opened** for `opened_s`, its shield
//!   and its armour gone. One member below the resistance cannot open it; several can, slowly.
//!
//! **S2's bite collapse** (HUNKER's diagnosis): a flight's hold read every threat in the
//! record, so a hunter lying still within `flee_stop_m` — or a flight pinned against a wall,
//! unable to get beyond it — kept the grazer fleeing for good, standing and never eating. Only
//! an approaching threat now holds a flight; a still one it can see lets the calm run. A flight
//! still for `still_after_s` against an obstruction runs along it, and a body in flight stays
//! aware of the hunter it flees (from the shelved S2b sprint work).

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::FloraView;
use serde::{Deserialize, Serialize};

use crate::perception::{HEARING, SIGHT, TREMOR, class};
use crate::{Actions, Animal, Fauna, FaunaConfig, Founder, MobilityMode, Percept, State, Track};

/// Which response a lineage's heuristics run on its record (a switch, no registry row).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VigilMode {
    /// None: its S1 reflexes, whatever its table.
    #[default]
    None,
    Herd,
    Arboreal,
    Freeze,
}

/// **A lineage's ground response** (module note): its knobs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Vigilance {
    /// The response (a switch).
    pub mode: VigilMode,
    /// Flight starts when threats are within this, metres.
    pub flee_start_m: f64,
    /// ...and stops beyond this of the threat it last knew (at least `flee_start_m`), metres.
    pub flee_stop_m: f64,
    /// ...once no threat has come within `flee_stop_m` for this long, seconds.
    pub calm_s: f64,
    /// An adult flees only from at least this many threats within `flee_start_m` (a
    /// juvenile from one): below it it stands its ground, facing them. Count.
    pub group_min: u32,
    /// Its emission (hearing's `speed^a × length^b`) is multiplied by this while it flees or
    /// bolts: a run is loud. Factor.
    pub run_noise: f64,
    /// Cropping between two scan pauses, seconds; 0 never scans.
    pub scan_every_s: f64,
    /// A scan pause, seconds.
    pub scan_s: f64,
    /// It faces a newly sensed threat this long, seconds; 0 never orients.
    pub orient_s: f64,
    /// A kin heard with an emission at least this many times its walking emission (cruise
    /// × length) is bolting, and it bolts too; 0 hears no alarm. Factor.
    pub alarm_emission_factor: f64,
    /// An alarm bolt lasts this long, seconds.
    pub alarm_bolt_s: f64,
    /// After a bolt it hears no alarm for this long, seconds.
    pub alarm_refractory_s: f64,
    /// Footfall (`left + right`, the tremor's units) that freezes it; 0 never freezes.
    pub freeze_tremor: f64,
    /// Footfall at which it also turns away in place.
    pub withdraw_tremor: f64,
    /// A freeze lasts this long after the last tremor that set it, seconds.
    pub freeze_s: f64,
    /// An arboreal body runs to a stand it can climb within this, metres.
    pub refuge_search_m: f64,
    /// **FEAR** (module note): it watches, not flees, a hunter it sees asleep or resting and
    /// not facing it (a switch, no registry row; off: S2's rule).
    pub watch_calm: bool,
    /// ...and walks off a watched hunter closer than this (3D), metres; 0 never walks off.
    pub calm_margin_m: f64,
    /// A resting hunter whose heading is within this of the bearing to it faces it: a
    /// threat, degrees.
    pub calm_facing_deg: f64,
    /// **FEAR-2** (module note): a flight, or a refuge watch, whose hunter it has not known
    /// within `flee_stop_m` for this long ends where it is, seconds; 0 never (S2's hold).
    pub lost_calm_s: f64,
}

impl Default for Vigilance {
    /// No response.
    fn default() -> Vigilance {
        Vigilance {
            mode: VigilMode::None,
            flee_start_m: 0.0,
            flee_stop_m: 0.0,
            calm_s: 0.0,
            group_min: 1,
            run_noise: 1.0,
            scan_every_s: 0.0,
            scan_s: 0.0,
            orient_s: 0.0,
            alarm_emission_factor: 0.0,
            alarm_bolt_s: 0.0,
            alarm_refractory_s: 0.0,
            freeze_tremor: 0.0,
            withdraw_tremor: 0.0,
            freeze_s: 0.0,
            refuge_search_m: 0.0,
            watch_calm: false,
            calm_margin_m: 0.0,
            calm_facing_deg: 0.0,
            lost_calm_s: 0.0,
        }
    }
}

/// **FEAR's watch** (module note), the same for every lineage that reads a sensed threat by
/// sight (the frondgrazer, the loftstrider, the seedporter), placeholders at the terrarium's
/// scale, untuned: the margin is 3 m, a little beyond the Veilhound's pack `commit_m` (2.5 m,
/// where its rush begins: a hunter that rises from beyond it is seen moving before it can
/// rush), about two of its body lengths; a resting hunter faces a prey whose bearing lies within
/// 30° of its heading (about a third of a predator's binocular field: looking at it, not past
/// it). **FEAR-2**: a hunter not known within the stop distance for 30 s (three calms: a
/// hunter that slips from view may still be near, so it waits longer than for one it saw go)
/// ends a flight or a refuge watch; a placeholder, untuned.
const WATCH: Vigilance = Vigilance { watch_calm: true, calm_margin_m: 3.0, calm_facing_deg: 30.0, lost_calm_s: 30.0, ..NONE };

/// [`Vigilance::default`] as a constant.
const NONE: Vigilance = Vigilance {
    mode: VigilMode::None,
    flee_start_m: 0.0,
    flee_stop_m: 0.0,
    calm_s: 0.0,
    group_min: 1,
    run_noise: 1.0,
    scan_every_s: 0.0,
    scan_s: 0.0,
    orient_s: 0.0,
    alarm_emission_factor: 0.0,
    alarm_bolt_s: 0.0,
    alarm_refractory_s: 0.0,
    freeze_tremor: 0.0,
    withdraw_tremor: 0.0,
    freeze_s: 0.0,
    refuge_search_m: 0.0,
    watch_calm: false,
    calm_margin_m: 0.0,
    calm_facing_deg: 0.0,
    lost_calm_s: 0.0,
};

/// **A lineage's hunker** (module note, HUNKER): its knobs. Inert unless `enabled` and its
/// vigilance is a herd's.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hunker {
    /// The switch (no registry row): off, the lineage keeps S2's flight.
    pub enabled: bool,
    /// It hunkers when an approaching threat is within this, metres.
    pub enter_m: f64,
    /// ...and comes out once none has been within this (at least `enter_m`) for the calm, metres.
    pub leave_m: f64,
    /// The calm at full condition, seconds.
    pub calm_s: f64,
    /// The calm is `calm_s` × condition^this (condition: reserve over cap): a hungrier body
    /// comes out sooner. 0: hunger does not shorten it. Exponent.
    pub hunger_calm_exp: f64,
    /// Below this condition it does not hunker (it cannot afford to stop eating). Fraction.
    pub min_condition: f64,
    /// The chance a strike on a full-grown, unwounded hunkered body glances off. Fraction.
    pub shield: f64,
    /// The shield grows as (body / adult body)^this: the young are soft. Exponent.
    pub shield_growth_exp: f64,
    /// A wound `w` takes `wound_opens × w` of the shield away. Factor.
    pub wound_opens: f64,
    /// One hunter in contact pries it open at this, progress per second (1 opens it).
    pub pry_per_member_s: f64,
    /// It resists (recovers) at this, progress per second: above one member's rate, one
    /// hunter alone cannot open it.
    pub resist_per_s: f64,
    /// A hunter is in contact within half their summed lengths plus this, metres.
    pub pry_reach_m: f64,
    /// Turned over, it is open (no shield, no armour) for this long, seconds.
    pub opened_s: f64,
    /// Kin within this is beside it: bunched, and (hunkered) a ring that resists, metres.
    pub ring_m: f64,
    /// Each hunkered kin within `ring_m` adds this × `resist_per_s`. Factor.
    pub ring_resist_per_kin: f64,
    /// ...counting at most this many kin. Count.
    pub ring_kin_max: u32,
}

impl Default for Hunker {
    /// No hunker.
    fn default() -> Hunker {
        Hunker {
            enabled: false,
            enter_m: 0.0,
            leave_m: 0.0,
            calm_s: 0.0,
            hunger_calm_exp: 0.0,
            min_condition: 0.0,
            shield: 0.0,
            shield_growth_exp: 0.0,
            wound_opens: 0.0,
            pry_per_member_s: 0.0,
            resist_per_s: 0.0,
            pry_reach_m: 0.0,
            opened_s: 0.0,
            ring_m: 0.0,
            ring_resist_per_kin: 0.0,
            ring_kin_max: 0,
        }
    }
}

impl Hunker {
    /// The chance a strike on a hunkered body of `body` (adult at `adult_body`) carrying
    /// `wound` glances off.
    pub fn shield_of(&self, body: f64, adult_body: f64, wound: f64) -> f64 {
        let grown = if adult_body > 0.0 { (body / adult_body).clamp(0.0, 1.0) } else { 1.0 };
        let hard = grown.powf(self.shield_growth_exp.max(0.0));
        (self.shield * hard * (1.0 - self.wound_opens * wound.max(0.0))).clamp(0.0, 1.0)
    }

    /// **Its resistance to prying** (PACK-TUNE), progress per second, at `body` (adult at
    /// `adult_body`) with `ringed` hunkered kin beside it: `resist_per_s` × (body over the
    /// adult body)^`shield_growth_exp` (the armour grows with the body, as the shield does: the
    /// young stay soft) × (1 + `ring_resist_per_kin` per kin, at most `ring_kin_max`).
    pub fn resist_of(&self, body: f64, adult_body: f64, ringed: u32) -> f64 {
        let grown = if adult_body > 0.0 { (body / adult_body).clamp(0.0, 1.0) } else { 1.0 };
        let hard = grown.powf(self.shield_growth_exp.max(0.0));
        self.resist_per_s * hard * (1.0 + self.ring_resist_per_kin * f64::from(ringed.min(self.ring_kin_max)))
    }

    /// The fewest hunters in contact that open it ([`Hunker::resist_of`] under their summed
    /// `pry_per_member_s`); `None` when no number does (no pry).
    pub fn members_to_open(&self, body: f64, adult_body: f64, ringed: u32) -> Option<u32> {
        if !(self.pry_per_member_s > 0.0) {
            return None;
        }
        let n = (self.resist_of(body, adult_body, ringed) / self.pry_per_member_s).floor() + 1.0;
        (n.is_finite() && n < f64::from(u32::MAX)).then_some(n as u32)
    }

    /// Its calm at `condition` (reserve over cap), in ticks.
    fn calm_ticks(&self, condition: f64) -> u64 {
        ticks(self.calm_s * condition.clamp(0.0, 1.0).powf(self.hunger_calm_exp.max(0.0)))
    }
}

/// **Each lineage's hunker** (HUNKER, Wrysk 2026-10-06), placeholders at the terrarium's
/// scale, untuned:
/// - the frondgrazer (*Tardigradus*, the slow-stepper under a three-plate saddle shield,
///   species dossier §D3): it clamps down with a Veilhound at 2.5 m (a little under twice its
///   length; the musk ox stands at a wolf's rush), comes out with none within 5 m for 20 s
///   (twice the flight's 10 s calm: a clamped animal waits out a circling pack); 0.9 of strikes
///   glance off a grown, unwounded shield (a tortoise's or an armadillo's carapace defeats
///   most bites); the shield grows as the square of the body fraction (a juvenile at half size
///   has a quarter of it). Turned over, open for 20 s. Kin within 1.5 m (about two body
///   lengths) bunch; each hunkered one beside it adds half its resistance, up to three (the
///   musk-ox ring). Below a tenth of its reserve it cannot afford to hunker.
///
///   **The pry** (PACK-TUNE, Wrysk: "one Veilhound can't open an adult; a pack can,
///   slowly"): n hunters in contact against resistance R open it in 1 / (n·p − R) s, and
///   never when n·p ≤ R. With R = 1/40 s⁻¹ and p = 1/90 s⁻¹ (R = 2.25 p): one or two never
///   (2/90 < 1/40), three in 1 / (3/90 − 1/40) = 120 s, four in 51 s. One hunkered kin beside
///   it (R × 1.5 = 3.375 p): three never, four in 144 s; two kin (4.5 p): four never. The
///   resistance grows with the body as the shield does ([`Hunker::resist_of`]): a half-grown
///   juvenile resists at R/4, so one hunter opens it in 206 s and two in 63 s. HUNKER's 1/60
///   let two open an adult in 120 s (57–378 opens a seed in 6 h). Armoured herds need several
///   hunters and still usually win: wolves rarely kill musk oxen standing in their defensive
///   group, and test and leave (Gray 1987, *The Muskoxen of Polar Bear Pass*; Mech 1970,
///   *The Wolf*); wolves' success on bison rises with pack size and stays low (MacNulty et
///   al. 2014, PLoS ONE 9:e112884); lions take buffalo only in larger groups, and a herd that
///   stands drives them off (Scheel & Packer 1991, Anim. Behav. 41:697; Packer & Ruttan 1988,
///   Am. Nat. 132:159).
pub fn hunker_of(founder: Founder) -> Hunker {
    match founder {
        Founder::Browser => Hunker {
            enabled: true,
            enter_m: 2.5,
            leave_m: 5.0,
            calm_s: 20.0,
            hunger_calm_exp: 1.0,
            min_condition: 0.1,
            shield: 0.9,
            shield_growth_exp: 2.0,
            wound_opens: 1.0,
            pry_per_member_s: 1.0 / 90.0,
            resist_per_s: 1.0 / 40.0,
            pry_reach_m: 0.3,
            opened_s: 20.0,
            ring_m: 1.5,
            ring_resist_per_kin: 0.5,
            ring_kin_max: 3,
        },
        _ => Hunker::default(),
    }
}

/// **Each lineage's ground response** (review §1 and §4, Astra's per-species rows),
/// placeholders at the terrarium's scale.
pub fn vigilance_of(founder: Founder) -> Vigilance {
    let none = Vigilance::default();
    match founder {
        // Astra: alternate cropping with brief scans; orient to uncertain sounds, flee a
        // confirmed approaching threat (start 4 m, half its 8 m sight), then reassess with
        // hysteresis (stop at its sight range, 10 s calm). A run is three times as loud as a
        // walk; a bolt of 3 s, deaf to alarms 20 s after.
        Founder::Browser => Vigilance {
            mode: VigilMode::Herd,
            flee_start_m: 4.0,
            flee_stop_m: 8.0,
            calm_s: 10.0,
            group_min: 1,
            run_noise: 3.0,
            scan_every_s: 6.0,
            scan_s: 1.5,
            orient_s: 3.0,
            alarm_emission_factor: 2.0,
            alarm_bolt_s: 3.0,
            alarm_refractory_s: 20.0,
            ..WATCH
        },
        // Panoramic from its eye ring, so no scan pause; an adult stands its ground against
        // a lone hunter and moves off before a converging group (two or more); no herd alarm.
        Founder::Loftstrider => Vigilance {
            mode: VigilMode::Herd,
            flee_start_m: 6.0,
            flee_stop_m: 12.0,
            calm_s: 10.0,
            group_min: 2,
            run_noise: 3.0,
            orient_s: 3.0,
            ..WATCH
        },
        // Flight to the trees: a threat within 8 m (half its daylight sight) on the ground
        // sends it to a stand within twice its roost reach (8 m); no alarm calls (D6, later).
        Founder::Seedporter => Vigilance {
            mode: VigilMode::Arboreal,
            flee_start_m: 8.0,
            flee_stop_m: 12.0,
            calm_s: 10.0,
            group_min: 1,
            run_noise: 3.0,
            refuge_search_m: 8.0,
            ..WATCH
        },
        // Freeze at the old flee line (predation's `flee_tremor`, 0.015), turn away at four
        // times it, 3 s.
        Founder::Blind => Vigilance {
            mode: VigilMode::Freeze,
            freeze_tremor: 0.015,
            withdraw_tremor: 0.06,
            freeze_s: 3.0,
            ..none
        },
        _ => none,
    }
}

/// **A body's response state**, kept in its record ([`Percept::vigil`]). Ticks are the
/// world's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vigil {
    /// At its last sample it was cropping (head down unless scanning).
    pub cropping: bool,
    /// A scan pause runs until this tick.
    pub scan_until: u64,
    /// The next scan pause begins at this tick of cropping (0: not yet scheduled).
    pub next_scan: u64,
    /// In flight, from the point it last knew a threat.
    pub fleeing: bool,
    pub from_x: f64,
    pub from_z: f64,
    /// The last tick an approaching threat was within `flee_stop_m`.
    pub close_tick: u64,
    /// The threat it flees (the nearest approaching one it knew), and the tick the flight
    /// began (S2b).
    pub flee_id: u64,
    pub flee_tick: u64,
    /// **HUNKER**: clamped down under its shield; the last tick an approaching threat was
    /// within `leave_m`; the pack's progress prying it open (0..1); open (turned over) until
    /// this tick.
    pub hunkered: bool,
    pub hunker_close: u64,
    pub pry: f64,
    pub opened_until: u64,
    /// An alarm bolt runs until this tick, along this heading.
    pub bolt_until: u64,
    pub bolt_heading: f64,
    /// It hears alarms again from this tick.
    pub alarm_ready: u64,
    /// The threat it oriented to (habituated to after), and until when it faces it.
    pub orient_id: u64,
    pub orient_until: u64,
    /// A freeze runs until this tick, turning `withdraw_turn` (0 still).
    pub freeze_until: u64,
    pub withdraw_turn: f64,
    /// **FEAR**: at its last sample it was walking off a watched hunter inside its margin.
    pub calm_walk: bool,
    /// **FEAR-2**: at its last sample it held its perch under a hunter (the refuge watch: in
    /// refuge, not in flight).
    pub refuge: bool,
}

impl Vigil {
    /// In a scan pause at `now`.
    pub fn scanning(&self, now: u64) -> bool {
        now < self.scan_until
    }

    /// Running (fleeing or bolting) at `now`: loud.
    pub fn running(&self, now: u64) -> bool {
        self.fleeing || now < self.bolt_until
    }

    /// Head down at `now`: cropping, not scanning and not running.
    pub fn head_down(&self, now: u64) -> bool {
        self.cropping && !self.scanning(now) && !self.running(now)
    }
}

fn ticks(s: f64) -> u64 {
    if s.is_finite() && s > 0.0 { cubarium_voxel::ticks_at_least_one(s) } else { 0 }
}

/// Steer toward the heading `want`: turn in place beyond 60°, else walk at `forward`.
fn head(a: &Animal, want: f64, forward: f64, feed: f64) -> Actions {
    let d = (want - a.pose.heading_rad).rem_euclid(std::f64::consts::TAU);
    let diff = if d > std::f64::consts::PI { d - std::f64::consts::TAU } else { d };
    Actions {
        forward: if diff.abs() > std::f64::consts::FRAC_PI_3 { 0.0 } else { forward },
        turn: (diff / std::f64::consts::FRAC_PI_4).clamp(-1.0, 1.0),
        feed,
    }
}

/// A threat in the record, from where it stands now.
#[derive(Clone, Copy, Debug)]
struct Threat {
    d: f64,
    x: f64,
    z: f64,
    id: u64,
    /// Seen moving or near, heard, or felt: an approaching body, not a still shape. Never
    /// for a watched one (`calm`).
    pressing: bool,
    /// **FEAR**: seen asleep, or resting and not facing it, by a lineage that watches.
    calm: bool,
    /// From it, 3D (its base to the track's base), metres.
    d3: f64,
}

/// **FEAR's judgement** of track `t` by the body at `a` with response `vg` (module note):
/// seen (only sight reads a posture), neither heard nor felt nor moving, and asleep, or
/// resting with its heading more than `calm_facing_deg` off the bearing to `a`.
fn calm_of(config: &FaunaConfig, vg: &Vigilance, a: &Animal, t: &Track, ring: f64) -> bool {
    use crate::perception::posture;
    if !vg.watch_calm || t.modes & SIGHT == 0 || t.modes & (HEARING | TREMOR) != 0 || t.moving {
        return false;
    }
    match t.posture {
        posture::ASLEEP => true,
        posture::REST => {
            let (dx, dz) = crate::pose::wrapped_delta(t.x, t.z, a.pose.x, a.pose.z, ring);
            // FEAR-2: awake in contact (within the predation table's `still_threat_near_m`),
            // it is a threat whichever way it faces: a hunter working at a hunkered grazer.
            if dx == 0.0 && dz == 0.0 || (dx * dx + dz * dz).sqrt() <= config.predation.still_threat_near_m {
                return false;
            }
            let d = (dx.atan2(dz) - t.heading).rem_euclid(std::f64::consts::TAU);
            let off = if d > std::f64::consts::PI { std::f64::consts::TAU - d } else { d };
            off > vg.calm_facing_deg.max(0.0).to_radians()
        }
        _ => false,
    }
}

fn threats(config: &FaunaConfig, a: &Animal, p: &Percept, now: u64, ring: f64, vm: f64) -> Vec<Threat> {
    let fresh = 2 * config.perception.stagger_ticks();
    let vg = a.founder.map(|f| config.founder(f).vigilance).unwrap_or_default();
    let base = crate::perception::base_m(a, vm);
    let mut out: Vec<Threat> = p
        .tracks()
        .iter()
        .filter(|t| t.class == class::THREAT && now.saturating_sub(t.tick) <= fresh)
        .map(|t| {
            let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, t.x, t.z, ring);
            let d = (dx * dx + dz * dz).sqrt();
            let seen_pressing = t.modes & SIGHT != 0 && (t.moving || d <= config.predation.still_threat_near_m);
            let calm = calm_of(config, &vg, a, t, ring);
            let dy = t.y_m - base;
            Threat {
                d,
                x: t.x,
                z: t.z,
                id: t.id,
                pressing: !calm && (seen_pressing || t.modes & (HEARING | TREMOR) != 0),
                calm,
                d3: (d * d + dy * dy).sqrt(),
            }
        })
        .collect();
    out.sort_by(|p, q| p.d.total_cmp(&q.d).then(p.id.cmp(&q.id)));
    out
}

/// **A readout** (FEAR-2's probe; no rule reads it): the threats the body `a` knows from its
/// record `p` at `now`, nearest first, each as `(id, distance on the ring, pressing as the herd
/// reads it, watched)`. The arboreal rule counts every unwatched one as pressing.
pub fn threat_readout(config: &FaunaConfig, a: &Animal, p: &Percept, now: u64, ring: f64, vm: f64) -> Vec<(u64, f64, bool, bool)> {
    threats(config, a, p, now, ring, vm).into_iter().map(|t| (t.id, t.d, t.pressing, t.calm)).collect()
}

/// The flight's hysteresis, shared by the herd and the arboreal: starts at `need` pressing
/// threats within `flee_start_m`, follows the nearest threat it knows, stops beyond
/// `flee_stop_m` of it after `calm_s` with none within. Returns whether it started now.
///
/// HUNKER's fix of S2's bite collapse: only an **approaching** threat (seen moving or near,
/// heard, felt) sets the point it flees and holds the flight; a threat it knows where it is
/// and sees still lets the calm run and ends the flight there, as being beyond `flee_stop_m`
/// of the point does. Before, a hunter lying still within `flee_stop_m` held it for good.
fn flight(v: &mut Vigil, vg: &Vigilance, a: &Animal, th: &[Threat], need: usize, now: u64, ring: f64) -> bool {
    let start = vg.flee_start_m.max(0.0);
    let stop = vg.flee_stop_m.max(start);
    let mut started = false;
    if !v.fleeing && th.iter().filter(|t| t.pressing && t.d <= start).count() >= need.max(1) {
        v.fleeing = true;
        v.close_tick = now;
        v.flee_tick = now;
        started = true;
    }
    if v.fleeing {
        if let Some(t) = th.iter().find(|t| t.pressing) {
            v.from_x = t.x;
            v.from_z = t.z;
            v.flee_id = t.id;
            if t.d <= stop {
                v.close_tick = now;
            }
        }
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, v.from_x, v.from_z, ring);
        let beyond = (dx * dx + dz * dz).sqrt() > stop;
        let seen_still = th.iter().any(|t| t.id == v.flee_id && !t.pressing);
        // FEAR-2: no hunter known within `flee_stop_m` for `lost_calm_s` (out of sight, a
        // body that cannot get beyond the stop distance of where it last saw it): over.
        let since = now.saturating_sub(v.close_tick);
        let lost = vg.lost_calm_s > 0.0 && since >= ticks(vg.lost_calm_s);
        if (beyond || seen_still || lost) && since >= ticks(vg.calm_s) {
            v.fleeing = false;
        }
    }
    started
}

/// Away from the point it flees, at full forward. **Cornered** (S2b: still for the predation
/// table's `still_after_s` this far into its flight, pressed against a wall or the strip's
/// end), it runs along the obstruction instead: voxel walls and the strip's ends lie along
/// the axes, so it takes the axis across its push, on the side away from the threat.
///
/// **EDGE**: the strip's end is no way out. Standing at an end (within its radius and half a
/// voxel), a heading out through it is taken along the end instead, on the side away from the
/// threat. And once cornered, or pressed for `teachers.press_replan_s` (held forward, its motor
/// refused: a body pushing hard can slide too much to count as still), the rest of that
/// flight goes by its lineage's route graph
/// ([`route_away`]): to the neighbouring node farthest from the point it flees, if any is
/// farther than its own; else it holds still, not pressing a wall it cannot pass. A body off
/// its graph keeps the axis rule.
fn away(fauna: &mut Fauna, view: &VoxelView<'_>, fv: &FloraView<'_>, a: &Animal, v: &Vigil, ring: f64, cornered: bool) -> Actions {
    let press = fauna.config.teachers.press_replan_s;
    let pressed = fauna.pressed.get(&a.id).is_some_and(|&t| press.is_finite() && press >= 0.0 && t >= cubarium_voxel::ticks_at_least_one(press));
    // EDGE-2: off its route graph with no walking way back on, it is cornered from the start:
    // its flight glides off its ledge where it can, else holds still, and never presses first.
    let pocket = fauna.route_flee.get(&a.id) != Some(&v.flee_tick)
        && matches!(crate::step::off_graph_exit(fauna, view, fv, a, (0.0, 0.0)), crate::step::OffGraph::Glide(_) | crate::step::OffGraph::Cornered);
    if cornered || pressed || pocket {
        fauna.route_flee.insert(a.id, v.flee_tick);
    }
    if fauna.route_flee.get(&a.id) == Some(&v.flee_tick) {
        if let Some(act) = route_away(fauna, view, fv, a, v, ring) {
            return act;
        }
    }
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, v.from_x, v.from_z, ring);
    let side = |d: f64| -> f64 {
        if d > 0.0 {
            -1.0
        } else if d < 0.0 {
            1.0
        } else if a.id % 2 == 0 {
            1.0
        } else {
            -1.0
        }
    };
    let want = if dx == 0.0 && dz == 0.0 {
        a.pose.heading_rad
    } else if cornered && dz.abs() >= dx.abs() {
        side(dx).atan2(0.0)
    } else if cornered {
        0.0f64.atan2(side(dz))
    } else {
        (-dx).atan2(-dz)
    };
    // The strip's ends: `forward` is `(sin h, cos h)`, so `cos` is the heading's z.
    let vm = view.config.voxel_m;
    let r = a.founder.map_or(0.0, |f| fauna.config.founder(f).body_at(a.body).footprint_radius());
    let (near, depth_m) = (r + 0.5 * vm, f64::from(view.config.depth) * vm);
    let want = if (a.pose.z <= near && want.cos() < 0.0) || (a.pose.z >= depth_m - near && want.cos() > 0.0) {
        side(dx).atan2(0.0)
    } else {
        want
    };
    head(a, want, 1.0, 0.0)
}

/// EDGE: a cornered body's step away on its lineage's route graph (`teachers.route_refresh_s`):
/// toward the neighbour of its node whose centre is farthest from the point it flees, when
/// that is farther than its own node's; else still. Off its graph, toward the first face of
/// its own body's way back onto it ([`crate::RouteGraph::way_on`], out to
/// `teachers.way_on_m`); with none, still.
fn route_away(fauna: &mut Fauna, view: &VoxelView<'_>, fv: &FloraView<'_>, a: &Animal, v: &Vigil, ring: f64) -> Option<Actions> {
    let founder = a.founder?;
    let tc = fauna.config.teachers;
    let graph = fauna.routes(view, founder, tc.route_refresh_s);
    let vm = view.config.voxel_m;
    let phys = *fauna.config.founder(founder);
    let body = phys.body_at(a.body);
    let r = body.footprint_radius();
    let own = (r / vm).ceil().max(0.0) as u32;
    let still = Actions { forward: 0.0, turn: 0.0, feed: 0.0 };
    let Some(from) = graph.node_near(a.site, (a.pose.x, a.pose.z), own) else {
        // EDGE-2: its own way back on; else, a glider, off its ledge away from the threat;
        // else still.
        let (ax, az) = crate::pose::wrapped_delta(v.from_x, v.from_z, a.pose.x, a.pose.z, ring);
        return Some(match crate::step::off_graph_exit(fauna, view, fv, a, (ax, az)) {
            crate::step::OffGraph::Way(f) => {
                let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, (f64::from(f.x) + 0.5) * vm, (f64::from(f.z) + 0.5) * vm, ring);
                head(a, dx.atan2(dz), 1.0, 0.0)
            }
            crate::step::OffGraph::Glide(h) => head(a, h, 1.0, 0.0),
            crate::step::OffGraph::On | crate::step::OffGraph::Cornered => still,
        });
    };
    let d_of = |(x, z): (f64, f64)| {
        let (dx, dz) = crate::pose::wrapped_delta(x, z, v.from_x, v.from_z, ring);
        dx * dx + dz * dz
    };
    let here = d_of(graph.centre(from));
    let best = graph
        .neighbours(from)
        .iter()
        .map(|&j| (d_of(graph.centre(j)), j))
        .filter(|&(d, _)| d > here)
        .max_by(|p, q| p.0.total_cmp(&q.0).then(q.1.cmp(&p.1)));
    Some(match best {
        Some((_, j)) => {
            let (cx, cz) = graph.centre(j);
            let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, cx, cz, ring);
            head(a, dx.atan2(dz), 1.0, 0.0)
        }
        None => still,
    })
}

/// Whether a body `flee_tick` into its flight has been still for `still_after_s` of it.
fn cornered(config: &FaunaConfig, a: &Animal, v: &Vigil, now: u64) -> bool {
    let still = config.predation.still_after_ticks().max(1);
    v.fleeing && now.saturating_sub(v.flee_tick) >= still && a.still_ticks >= still
}

/// **FEAR's walk off** (module note): away from the nearest watched hunter within its
/// `calm_margin_m` (3D), at its walk (forward 1, no sprint), not feeding; at the strip's end,
/// along it, as [`away`] does. `None` with none that near, or once its motor has refused the
/// walk for `teachers.press_replan_s` (it does not press a wall to keep the margin).
fn calm_walk(fauna: &Fauna, view: &VoxelView<'_>, a: &Animal, vg: &Vigilance, th: &[Threat], ring: f64) -> Option<Actions> {
    if !(vg.calm_margin_m > 0.0) || a.mobility.mode != MobilityMode::Ground {
        return None;
    }
    let t = th.iter().filter(|t| t.calm && t.d3 <= vg.calm_margin_m).min_by(|p, q| p.d3.total_cmp(&q.d3).then(p.id.cmp(&q.id)))?;
    let press = fauna.config.teachers.press_replan_s;
    if fauna.pressed.get(&a.id).is_some_and(|&n| press.is_finite() && press >= 0.0 && n >= cubarium_voxel::ticks_at_least_one(press)) {
        return None;
    }
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, t.x, t.z, ring);
    let want = if dx == 0.0 && dz == 0.0 { a.pose.heading_rad } else { (-dx).atan2(-dz) };
    let vm = view.config.voxel_m;
    let r = a.founder.map_or(0.0, |f| fauna.config.founder(f).body_at(a.body).footprint_radius());
    let (near, depth_m) = (r + 0.5 * vm, f64::from(view.config.depth) * vm);
    let side = if dx > 0.0 { -1.0 } else if dx < 0.0 { 1.0 } else if a.id % 2 == 0 { 1.0 } else { -1.0 };
    let want = if (a.pose.z <= near && want.cos() < 0.0) || (a.pose.z >= depth_m - near && want.cos() > 0.0) { f64::atan2(side, 0.0) } else { want };
    Some(head(a, want, 1.0, 0.0))
}

/// Its condition: reserve over its cap at its body.
fn condition_of(config: &FaunaConfig, a: &Animal) -> f64 {
    let Some(f) = a.founder else { return 1.0 };
    let cap = config.founder(f).core.reserve_of(a.body);
    if cap > 0.0 { (a.reserve / cap).clamp(0.0, 1.0) } else { 0.0 }
}

/// The kin it knows (its record's fresh kin tracks): their mean point, and the distance to
/// the nearest; `None` with none.
fn kin_of(config: &FaunaConfig, a: &Animal, p: &Percept, now: u64, ring: f64) -> Option<((f64, f64), f64)> {
    let fresh = 2 * config.perception.stagger_ticks();
    let (mut sx, mut sz, mut n, mut nearest) = (0.0, 0.0, 0usize, f64::INFINITY);
    for t in p.tracks().iter().filter(|t| t.class == class::KIN && t.id != a.id && now.saturating_sub(t.tick) <= fresh) {
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, t.x, t.z, ring);
        sx += dx;
        sz += dz;
        n += 1;
        nearest = nearest.min((dx * dx + dz * dz).sqrt());
    }
    (n > 0).then(|| ((sx / n as f64, sz / n as f64), nearest))
}

/// **The ground response** of the body at `i` (module note), for a live lineage with a mode;
/// `None` leaves its heuristic's actions. `held` is what its heuristic asked this sample.
pub(crate) fn respond(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, held: Actions) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    if !phys.senses.live {
        return None;
    }
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    match phys.vigilance.mode {
        VigilMode::None => None,
        VigilMode::Herd => herd(fauna, i, view, fv, ring, held),
        VigilMode::Arboreal => arboreal(fauna, i, view, fv, ring, held),
        VigilMode::Freeze => freeze(fauna, i),
    }
}

fn herd(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, ring: f64, held: Actions) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let vg = phys.vigilance;
    let hk = phys.hunker;
    let now = fauna.tick;
    let p = fauna.percepts.get(&a.id).copied().unwrap_or_default();
    let mut v = p.vigil;
    v.calm_walk = false;
    let th = threats(&fauna.config, &a, &p, now, ring, view.config.voxel_m);
    let adult = a.body >= phys.core.birth_body;
    let need = if adult { vg.group_min.max(1) as usize } else { 1 };
    let k = founder.index();
    let kin = if hk.enabled { kin_of(&fauna.config, &a, &p, now, ring) } else { None };
    // HUNKER: clamp down at an approaching threat within `enter_m`; come out once none has been
    // within `leave_m` for the calm, which hunger shortens.
    if hk.enabled {
        let enter = hk.enter_m.max(0.0);
        let leave = hk.leave_m.max(enter);
        let cond = condition_of(&fauna.config, &a);
        if !v.hunkered && cond >= hk.min_condition && th.iter().any(|t| t.pressing && t.d <= enter) {
            v.hunkered = true;
            v.hunker_close = now;
            v.pry = 0.0;
            v.fleeing = false;
            v.bolt_until = now;
            fauna.ledger.vigil.hunkers[k] += 1;
        }
        if v.hunkered {
            if th.iter().any(|t| t.pressing && t.d <= leave) {
                v.hunker_close = now;
            }
            if now.saturating_sub(v.hunker_close) >= hk.calm_ticks(cond) {
                // The turn-over ends with the hunker: a later hunker starts under its shield.
                v.hunkered = false;
                v.pry = 0.0;
                v.opened_until = 0;
            }
        }
        v.fleeing = false;
    } else {
        v.hunkered = false;
        if flight(&mut v, &vg, &a, &th, need, now, ring) {
            fauna.ledger.vigil.flees[k] += 1;
        }
    }
    if p.vigil.hunkered && !v.hunkered && now < p.vigil.opened_until {
        fauna.ledger.vigil.open_exits[k] += 1;
    }
    if !p.vigil.hunkered && v.hunkered && now < v.opened_until {
        fauna.ledger.vigil.bared_entries[k] += 1;
    }
    fauna.animals[i].hunkered = v.hunkered;
    if v.hunkered {
        v.cropping = false;
        fauna.percepts.entry(a.id).or_default().vigil = v;
        return Some(Actions { forward: 0.0, turn: 0.0, feed: 0.0 });
    }
    // HUNKER's bunching: a threat within `flee_start_m` sends a body whose kin are farther
    // than `ring_m` running to them (loud: the run alarms the rest).
    if hk.enabled && now >= v.bolt_until {
        if let Some(((kx, kz), nearest)) = kin {
            if nearest > hk.ring_m && th.iter().any(|t| t.pressing && t.d <= vg.flee_start_m) {
                v.bolt_until = now + ticks(vg.alarm_bolt_s).max(1);
                v.bolt_heading = kx.atan2(kz);
                fauna.ledger.vigil.bunches[k] += 1;
            }
        }
    }
    // The herd alarm: a kin heard running.
    if !v.fleeing && now >= v.alarm_ready && vg.alarm_emission_factor > 0.0 && now >= v.bolt_until {
        let pc = fauna.config.perception;
        let walk = |size: f64| (phys.cruise_body_lengths_per_s * size).max(0.0).powf(pc.hearing_speed_exp) * size.max(0.0).powf(pc.hearing_size_exp);
        let bolting = p.tracks().iter().filter(|t| t.class == class::KIN && t.modes & HEARING != 0 && t.tick + 2 * pc.stagger_ticks() >= now).find(|t| {
            let heard = t.loud * t.d_m.max(pc.hearing_near_m);
            heard >= vg.alarm_emission_factor * walk(t.size_m)
        });
        if let Some(t) = bolting {
            v.bolt_until = now + ticks(vg.alarm_bolt_s).max(1);
            // HUNKER: toward the kin it knows, not along the runner's heading.
            v.bolt_heading = match kin {
                Some(((kx, kz), _)) => kx.atan2(kz),
                None => t.heading,
            };
            v.alarm_ready = v.bolt_until + ticks(vg.alarm_refractory_s);
            fauna.ledger.vigil.alarm_bolts[k] += 1;
        }
    }
    let mut out = None;
    if v.fleeing {
        let cornered = cornered(&fauna.config, &a, &v, now);
        out = Some(away(fauna, view, fv, &a, &v, ring, cornered));
    } else if now < v.bolt_until {
        // A bunching body already beside its kin stands, head up, facing them.
        let beside = hk.enabled && kin.is_some_and(|(_, nearest)| nearest <= hk.ring_m);
        out = Some(head(&a, v.bolt_heading, if beside { 0.0 } else { 1.0 }, 0.0));
    } else if vg.orient_s > 0.0 {
        // A threat it has not faced: face it. Below its group it stands its ground, facing
        // the nearest pressing one.
        let standing = th.iter().find(|t| t.pressing && t.d <= vg.flee_start_m);
        if let Some(t) = th.iter().find(|t| t.pressing && t.id != v.orient_id).filter(|_| now >= v.orient_until) {
            v.orient_id = t.id;
            v.orient_until = now + ticks(vg.orient_s);
            fauna.ledger.vigil.orients[k] += 1;
        }
        let facing = if now < v.orient_until { th.iter().find(|t| t.id == v.orient_id) } else { None };
        if let Some(t) = facing.or(standing) {
            let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, t.x, t.z, ring);
            let mut act = head(&a, dx.atan2(dz), 0.0, 0.0);
            act.forward = 0.0;
            out = Some(act);
        } else if th.iter().all(|t| t.id != v.orient_id) {
            v.orient_id = 0;
        }
    }
    // FEAR: inside its margin of a watched hunter, it walks off.
    if out.is_none() && !v.fleeing && now >= v.bolt_until {
        if let Some(act) = calm_walk(fauna, view, &a, &vg, &th, ring) {
            v.calm_walk = true;
            out = Some(act);
        }
    }
    // The scan pause, while cropping and nothing else holds it.
    let cropping = held.feed > 0.5 && held.forward <= fauna.config.teachers.forage_stay_forward;
    v.cropping = out.is_none() && cropping;
    if out.is_none() && vg.scan_every_s > 0.0 && vg.scan_s > 0.0 {
        let every = ticks(vg.scan_every_s).max(1);
        if cropping && !v.scanning(now) {
            if v.next_scan == 0 {
                v.next_scan = now + 1 + a.id % every;
            }
            if now >= v.next_scan {
                v.scan_until = now + ticks(vg.scan_s).max(1);
                v.next_scan = v.scan_until + every;
                fauna.ledger.vigil.scans[k] += 1;
            }
        }
        if v.scanning(now) {
            v.cropping = false;
            out = Some(Actions { forward: 0.0, turn: 0.0, feed: 0.0 });
        }
    }
    fauna.percepts.entry(a.id).or_default().vigil = v;
    out
}

fn arboreal(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, ring: f64, held: Actions) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let vg = phys.vigilance;
    let now = fauna.tick;
    let p = fauna.percepts.get(&a.id).copied().unwrap_or_default();
    let mut v = p.vigil;
    v.calm_walk = false;
    let th = threats(&fauna.config, &a, &p, now, ring, view.config.voxel_m);
    let k = founder.index();
    // Any threat it senses counts, still or moving: it is a small body on the ground. FEAR: not
    // one it watches (asleep, or resting and not facing it).
    let sensed: Vec<Threat> = th.iter().map(|t| Threat { pressing: !t.calm, ..*t }).collect();
    let grounded = a.mobility.mode == MobilityMode::Ground;
    // FEAR-2: a perch is its refuge. A flight ends on reaching one, and the refuge watch keeps
    // it there (no step down, no glide; it turns and feeds as it asks) while a hunter it does
    // not watch is within `flee_start_m`, then within `flee_stop_m`, and `lost_calm_s` (at
    // least `calm_s`) after the last.
    if a.mobility.mode == MobilityMode::Perched {
        v.fleeing = false;
        let (start, stop) = (vg.flee_start_m.max(0.0), vg.flee_stop_m.max(vg.flee_start_m.max(0.0)));
        let reach = if v.refuge { stop } else { start };
        if sensed.iter().any(|t| t.pressing && t.d <= reach) {
            v.refuge = true;
            v.close_tick = now;
        }
        if v.refuge && now.saturating_sub(v.close_tick) >= ticks(vg.lost_calm_s.max(vg.calm_s)) {
            v.refuge = false;
        }
        let out = v.refuge.then_some(Actions { forward: 0.0, turn: held.turn, feed: held.feed });
        fauna.percepts.entry(a.id).or_default().vigil = v;
        return out;
    }
    v.refuge = false;
    if flight(&mut v, &vg, &a, &sensed, 1, now, ring) && grounded {
        fauna.ledger.vigil.flees[k] += 1;
    }
    let mut out = None;
    if v.fleeing {
        out = match a.mobility.mode {
            MobilityMode::Ground => {
                match crate::step::refuge_step(fauna, i, view, fv, vg.refuge_search_m) {
                    Some(go) => {
                        fauna.ledger.vigil.refuges[k] += 1;
                        Some(crate::gait::dash(&fauna.config, founder, v.close_tick == now, go))
                    }
                    None => {
                        let cornered = cornered(&fauna.config, &a, &v, now);
                        let go = away(fauna, view, fv, &a, &v, ring, cornered);
                        Some(crate::gait::dash(&fauna.config, founder, v.close_tick == now, go))
                    }
                }
            }
            // Up the stem (in the crown, the refuge watch above).
            MobilityMode::Climb if !a.mobility.falling => Some(Actions { forward: 1.0, turn: 0.0, feed: 0.0 }),
            _ => None,
        };
    } else if let Some(act) = calm_walk(fauna, view, &a, &vg, &th, ring) {
        // FEAR: inside its margin of a watched hunter, it walks off (no dash, no refuge).
        v.calm_walk = true;
        out = Some(act);
    }
    fauna.percepts.entry(a.id).or_default().vigil = v;
    out
}

fn freeze(fauna: &mut Fauna, i: usize) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let vg = fauna.config.founder(founder).vigilance;
    let now = fauna.tick;
    let p = fauna.percepts.get(&a.id).copied().unwrap_or_default();
    let mut v = p.vigil;
    let [left, right] = p.tremor;
    let felt = left + right;
    if vg.freeze_tremor > 0.0 && felt >= vg.freeze_tremor {
        if now >= v.freeze_until {
            fauna.ledger.vigil.freezes[founder.index()] += 1;
        }
        v.freeze_until = now + ticks(vg.freeze_s).max(1);
        v.withdraw_turn = if vg.withdraw_tremor > 0.0 && felt >= vg.withdraw_tremor {
            if left > right { 1.0 } else { -1.0 }
        } else {
            0.0
        };
    }
    let out = (now < v.freeze_until).then_some(Actions { forward: 0.0, turn: v.withdraw_turn, feed: 0.0 });
    fauna.percepts.entry(a.id).or_default().vigil = v;
    out
}

/// **The strike's awareness** (D5): for a prey whose lineage is live, the predation table's
/// `awareness_sensed` when its record holds the striker sensed within `aware_within_s`, else
/// `awareness_unaware` (asleep, the table's asleep value); for any other prey the table's
/// state column, as before.
pub fn awareness(config: &FaunaConfig, prey: &Animal, record: Option<&Percept>, striker: u64, now: u64) -> f64 {
    let cfg = &config.predation;
    let live = prey.founder.is_some_and(|f| config.founder(f).senses.live);
    if !live || prey.state == crate::State::Asleep {
        return cfg.awareness(prey.state);
    }
    if aware_of(config, record, striker, now) { cfg.awareness_sensed } else { cfg.awareness_unaware }
}

/// Whether a record holds `striker` sensed within the predation table's `aware_within_s`, or
/// (S2b) the body is in flight from it: a prey running from a hunter knows where it is,
/// though it has it behind, outside its field.
pub fn aware_of(config: &FaunaConfig, record: Option<&Percept>, striker: u64, now: u64) -> bool {
    let within = ticks(config.predation.aware_within_s);
    let fresh = |t: &Track| t.known && t.id == striker && now.saturating_sub(t.tick) <= within;
    record.is_some_and(|p| {
        p.tracks().iter().any(fresh)
            || fresh(&p.best[usize::from(class::THREAT)])
            || (p.vigil.fleeing && p.vigil.flee_id == striker && striker != 0)
    })
}

/// **What a strike meets** (HUNKER): the prey's neck out, its shield, or its bared underside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guard {
    /// Not hunkered: the dodge and the armour as before.
    Open,
    /// Hunkered under its shield: no dodge, and the strike glances off with this chance.
    Shield(f64),
    /// Hunkered and turned over by the pack: no dodge, no shield, no armour.
    Bared,
}

/// **The guard** a strike on `prey` meets at `now` (HUNKER, module note).
pub fn guard(config: &FaunaConfig, prey: &Animal, record: Option<&Percept>, now: u64) -> Guard {
    let Some(f) = prey.founder else { return Guard::Open };
    let phys = config.founder(f);
    let hk = phys.hunker;
    let Some(v) = record.map(|p| p.vigil) else { return Guard::Open };
    if !hk.enabled || !v.hunkered {
        return Guard::Open;
    }
    if now < v.opened_until {
        return Guard::Bared;
    }
    Guard::Shield(hk.shield_of(prey.body, phys.core.birth_body, prey.wound))
}

/// Whether body `id` is hunkered and turned over (open) now.
pub fn opened(fauna: &Fauna, id: u64) -> bool {
    fauna.percept(id).is_some_and(|p| p.vigil.hunkered && fauna.tick < p.vigil.opened_until)
}

/// **The pack's work** (HUNKER, module note), once a tick: every hunkered body of a lineage
/// with a hunker is pried at by the awake, grown hunters of it in contact, and resists; at
/// progress 1 it is turned over (open for `opened_s`). The readouts: hunkered ticks, worked
/// ticks and the members working, opens.
pub(crate) fn pry(fauna: &mut Fauna, ring: f64) {
    let now = fauna.tick;
    let dt = cubarium_voxel::DT;
    let worked: Vec<(usize, Founder)> = fauna
        .animals
        .iter()
        .enumerate()
        .filter_map(|(j, p)| {
            let f = p.founder?;
            (fauna.config.founder(f).hunker.enabled && p.hunkered).then_some((j, f))
        })
        .collect();
    for (j, f) in worked {
        let p = fauna.animals[j];
        let hk = fauna.config.founder(f).hunker;
        let Some(pl) = crate::body_of(&fauna.config, &p).map(|b| b.length_m) else { continue };
        let mut members = 0u32;
        let mut ringed = 0u32;
        for o in fauna.animals.iter() {
            if o.id == p.id || o.state == State::Asleep || o.attached.is_some() {
                continue;
            }
            let Some(of) = o.founder else { continue };
            let (dx, dz) = crate::pose::wrapped_delta(p.pose.x, p.pose.z, o.pose.x, o.pose.z, ring);
            let d = (dx * dx + dz * dz).sqrt();
            if of == f {
                if o.hunkered && d <= hk.ring_m {
                    ringed += 1;
                }
                continue;
            }
            let hunts = fauna.config.founder(of).hunter.is_some_and(|h| h.hunts(f));
            if !hunts || o.pack.is_some_and(|m| m.pup) {
                continue;
            }
            let Some(ol) = crate::body_of(&fauna.config, o).map(|b| b.length_m) else { continue };
            if d <= 0.5 * (pl + ol) + hk.pry_reach_m {
                members += 1;
            }
        }
        let adult = fauna.config.founder(f).core.birth_body;
        let resist = hk.resist_of(p.body, adult, ringed);
        let l = &mut fauna.ledger.vigil;
        l.hunker_ticks[f.index()] += 1;
        if members > 0 {
            l.worked_ticks[f.index()] += 1;
            l.worker_ticks[f.index()] += u64::from(members);
        }
        let v = &mut fauna.percepts.entry(p.id).or_default().vigil;
        if now < v.opened_until {
            continue;
        }
        v.pry = (v.pry + dt * (f64::from(members) * hk.pry_per_member_s - resist)).max(0.0);
        if v.pry >= 1.0 {
            v.pry = 0.0;
            v.opened_until = now + ticks(hk.opened_s).max(1);
            fauna.ledger.vigil.opens[f.index()] += 1;
        }
    }
}

/// **The stalk** (the Veilhound's gait, review §1 and §4): the forward a pack member at `a`
/// closing on the prey it knows as `t`, `d` metres off, may use. Beyond its pack's
/// `commit_m` it stalks: no faster than keeps its emission under `stalk_margin` of the
/// prey's hearing floor at that distance, and still while the prey looks up (a scan it
/// sees, with `stalk_freeze`); within it, or once the prey runs, it rushes (`asked`).
/// `None` when the pack does not stalk.
pub(crate) fn stalk(fauna: &mut Fauna, a: &Animal, t: &Track, d: f64, asked: f64) -> Option<f64> {
    let pk = fauna.config.founder(a.founder?).pack?;
    if !(pk.stalk_margin > 0.0) {
        return None;
    }
    let now = fauna.tick;
    let prey = fauna.view().animal(t.id).copied()?;
    let pv = fauna.percept(prey.id).map(|p| p.vigil).unwrap_or_default();
    if d <= pk.commit_m || pv.running(now) {
        fauna.ledger.vigil.rush_samples += 1;
        // GAIT: the rush is its sprint.
        return Some(asked * crate::gait::sprint_forward(&fauna.config, a.founder?));
    }
    if pk.stalk_freeze && t.modes & SIGHT != 0 && pv.scanning(now) {
        fauna.ledger.vigil.stalk_freezes += 1;
        return Some(0.0);
    }
    let pt = fauna.config.founder(prey.founder?).senses;
    let pc = fauna.config.perception;
    let geometry = crate::body_of(&fauna.config, a)?;
    let cruise = fauna.config.manifest(a.founder?).cruise_m_per_s;
    if !(pt.hearing_m > 0.0) || d > pt.hearing_m || !(cruise > 0.0) || !(pc.hearing_speed_exp > 0.0) {
        return Some(asked);
    }
    fauna.ledger.vigil.stalk_samples += 1;
    let emission = pk.stalk_margin * pt.hearing_min * d.max(pc.hearing_near_m) / geometry.length_m.max(1e-9).powf(pc.hearing_size_exp);
    let speed = emission.max(0.0).powf(1.0 / pc.hearing_speed_exp);
    Some(asked.min(speed / cruise))
}

/// **The response readouts** (S2): counters only, no rule reads them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VigilLedger {
    /// Scan pauses, flights begun, alarm bolts, orientations, freezes and refuge samples
    /// (an arboreal body walking to a tree in flight), by lineage.
    pub scans: [u64; Founder::COUNT],
    pub flees: [u64; Founder::COUNT],
    pub alarm_bolts: [u64; Founder::COUNT],
    pub orients: [u64; Founder::COUNT],
    pub freezes: [u64; Founder::COUNT],
    pub refuges: [u64; Founder::COUNT],
    /// Strikes that met a prey of a live lineage that had sensed the striker, and that had
    /// not, by the prey's lineage.
    pub aware_strikes: [u64; Founder::COUNT],
    pub unaware_strikes: [u64; Founder::COUNT],
    /// Pack chase samples stalking, rushing, and frozen by a prey's scan.
    pub stalk_samples: u64,
    pub rush_samples: u64,
    pub stalk_freezes: u64,
    /// HUNKER, by the prey's lineage: hunkers begun, bunching runs begun, ticks hunkered,
    /// hunkered ticks with at least one hunter in contact and the hunters in contact summed
    /// over them, turn-overs (opens), strikes that glanced off a shield; kills (by any hunter)
    /// of juveniles (under the adult body) and of adults.
    pub hunkers: [u64; Founder::COUNT],
    pub bunches: [u64; Founder::COUNT],
    pub hunker_ticks: [u64; Founder::COUNT],
    pub worked_ticks: [u64; Founder::COUNT],
    pub worker_ticks: [u64; Founder::COUNT],
    pub opens: [u64; Founder::COUNT],
    pub glances: [u64; Founder::COUNT],
    pub kills_juvenile: [u64; Founder::COUNT],
    pub kills_adult: [u64; Founder::COUNT],
    /// FINDINGS-FIX readouts: hunkers ended while still turned over (`opened_until` ahead),
    /// and hunkers begun bared (an earlier turn-over's `opened_until` still ahead).
    #[serde(default)]
    pub open_exits: [u64; Founder::COUNT],
    #[serde(default)]
    pub bared_entries: [u64; Founder::COUNT],
}
