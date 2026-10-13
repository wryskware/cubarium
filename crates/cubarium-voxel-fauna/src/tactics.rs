//! **VH-TACTICS: a Veilhound pack that tests, cuts off, encircles, harries and gives up**
//! (`design/behaviour-ai-2026-10-11.md` package 5: §2.1, §2.2, §4, §11.2, §11.4–§11.6; the
//! brief `design/handoffs/vh-tactics-2026-10-12.md`).
//!
//! A child of [`crate::pack`]: it reuses the pack's walking, routing and strike-approach rules
//! (`route_go`, `route_step`, `close_in`, `way_home`, `keep_out`). With [`Tactics::enabled`]
//! (the Veilhound's default) [`crate::pack::steer`] runs [`tactic`] in place of the old pack
//! tactic, which stays behind the switch as the comparison arm.
//!
//! # What a hound reads (§2.1, §11.2)
//!
//! Its own body (pose, size, wound, reserve, stamina), its own percept record (prey, kin and
//! threat tracks with what sight saw of them, the carcasses it scanned, the scent), the calls
//! it heard ([`crate::calls`]), its den (a home place it always knows), the other packs' dens
//! (their scent-marked territories) and its own tactical memory ([`Hound`]). Never another
//! body's record or hunger, a packmate it does not sense, a true position, the pack's target
//! or feast. What it sees of a sensed body is read off the track (size, lame, posture,
//! heading, moving) or, for a visible act, off that body only while a sight track of it is
//! fresh: a kick wound up or recovering ([`observe_kicks`]).
//!
//! # The intent layer (§2.2, §4.2)
//!
//! Each controller sample a hungry hound weighs the Veilhound menu's intents
//! ([`crate::intent::VEILHOUND`]) from its scene and holds one with the shared rule
//! ([`crate::intent::arbitrate`]: `hold_s`, then `switch_margin` on probabilities at the
//! lineage's temperature). Its proposal (the best before the hold) and its held intent go to
//! the intent record ([`HoundTeach`]): these are the brains' teachers later.
//! - **search** (nothing to hunt): a hunt cue it heard, a regroup howl, the scent, the nearest
//!   packmate it sees, the patrol;
//! - **stalk** (a lone prey not yet taken): close under the stalk's quiet until the rush;
//! - **test** (a group, nobody taken yet): approach openly to `test_m`, watch, a short rush,
//!   then commit to the best of the group or break off (§4.3: the test is where the choice is
//!   made);
//! - **drive**, **cut_off**, **intercept**, **encircle**, **harry** (committed): scored and
//!   held;
//! - **give_up**: the value fell under `give_up_value`, or its wound, stamina or time without
//!   progress passed a knob: break off, shun it, howl, go home or walk off and rest;
//! - **feed**, **rest**, **den**: a carcass it knows, the rhythm, the den walk.
//!
//! # Target value (§4.3)
//!
//! `value = meat × p_kill − chase cost − risk`, all from what the hound senses ([`assess`]).
//! Bookkeeping (the pack's target, feast and chase events, the census) is written for the
//! readouts and read by no rule here ([`bookkeep_start`], [`bookkeep_end`]).

use std::f64::consts::{FRAC_PI_2, PI};

use serde::{Deserialize, Serialize};

use super::*;
use crate::intent::{Subject, VEILHOUND};
use crate::perception::{SIGHT, posture};

// ------------------------------------------------------------------ knobs

/// **The Veilhound's tactics** ([`crate::Pack::tactics`]). Value is in meals of the hound's
/// own size (a prey of its own length is 1). Placeholders, untuned: the topic-8 target (a
/// rare feast) is reached by turning them, not asserted.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tactics {
    /// On: this module's tactic. Off: the old pack tactic (the comparison arm). A switch.
    pub enabled: bool,
    /// **Meat**: a prey of length `l` is worth `(l / own length)^meat_exp` (mass goes as
    /// length cubed).
    pub meat_exp: f64,
    /// **Meat by mass** (VH-FIX): on, a prey is worth the organic matter its lineage carries
    /// at the length the hound sees (the lineage's `body_max × (length / adult length)³`) over
    /// the hound's own body — the same unit as its gut room. Off: `meat_exp` on lengths. A
    /// switch.
    pub meat_by_mass: bool,
    /// **Usable meals** (VH-FIX): on, the meat counted is at most what the hounds at hand can
    /// eat before the carcass is gone ([`Me::usable`] × the hounds): their gut room now plus
    /// what the gut takes over `carcass_keep_s`, bounded by what their reserves and upkeep
    /// can absorb. A carcass bigger than that is worth no more than that. A switch.
    pub usable_meals: bool,
    /// ...the time a kill stays worth returning to, as a hound judges it, seconds (about the
    /// carrion's e-folding time: 1 / `flora.carrion_decomposition`).
    pub carcass_keep_s: f64,
    /// **Chase cost**: value spent per metre between the hound and the prey.
    pub chase_cost_per_m: f64,
    /// **`p_kill`**, one hound's chance on a grown, sound, alert prey of its own length with
    /// no factor applied, 0..1. The factors below (each "×") multiply its odds, `p / (1 − p)`,
    /// so none saturates at certainty.
    pub p_one: f64,
    /// ...× `(own length / prey length)^size_exp`: a bigger body is harder to pull down.
    pub size_exp: f64,
    /// A body shorter than this share of its lineage's adult length looks young.
    pub young_length_fraction: f64,
    /// ...× `(1 + young_gain × (1 − length / adult length))`: the younger, the easier.
    pub young_gain: f64,
    /// ...× `(1 + lame_gain)` for a body it sees limp.
    pub lame_gain: f64,
    /// ...× `(1 + isolation_gain × min(d, isolation_m) / isolation_m)`, d the distance from
    /// the prey to the nearest grown body of its lineage the hound senses (none: the full
    /// gain). Metres, factor.
    pub isolation_m: f64,
    pub isolation_gain: f64,
    /// ...× `(1 + resting_gain)` for a body it sees lying or asleep.
    pub resting_gain: f64,
    /// ...× `(1 + fleeing_gain)` for a body it sees running away from it.
    pub fleeing_gain: f64,
    /// Grown bodies of the prey's lineage within this of it guard it, metres.
    pub guard_m: f64,
    /// ...each guard: × `adult_factor` (numbers), counting at most `adults_max`.
    pub adult_factor: f64,
    pub adults_max: u32,
    /// ...each grown body (guards and the prey itself) it sees facing a hound within
    /// `facing_deg`: × `facing_factor`.
    pub facing_factor: f64,
    pub facing_deg: f64,
    /// ...each guard in guard stance: × `guard_factor`. No lineage shows a guard stance yet
    /// (LS-DEFEND gives the loftstrider one): reads none until then.
    pub guard_factor: f64,
    /// **Risk**: a grown body at least this many times the hound's length kicks (a hoofed
    /// adult; a calf or a grazer does not).
    pub kicker_length_ratio: f64,
    /// ...each kicker (the prey and its guards) is worth this many expected kicks, × `(1 +
    /// kick_facing_gain)` facing a hound, × `(1 + kick_guard_gain)` in guard stance, and none
    /// while it recovers from a kick the hound saw ([`Tactics::kick_recovery_s`]).
    pub kick_chance: f64,
    pub kick_facing_gain: f64,
    pub kick_guard_gain: f64,
    /// ...an expected kick costs this much value, × `(1 + own wound)`, × `(1 − hunger_relief
    /// × hunger)` (hunger is 1 − its reserve fraction): a starving hound risks more.
    pub wound_cost: f64,
    pub hunger_relief: f64,
    /// A kicker's reach as a hound judges it, as a share of the kicker's length.
    pub kick_reach_fraction: f64,
    /// A kick it saw counts for this long (it keeps farther off that body), seconds.
    pub kick_memory_s: f64,
    /// ...and the kicker is taken as recovering for this long after it (no kick expected),
    /// seconds. Only a kick the hound saw (or felt land on itself: [`felt_kick`]); otherwise
    /// unknown.
    pub kick_recovery_s: f64,
    /// **VH-FEED-KICK: keep clear of a kicker** ([`keep_clear`]). On: a grown hound that senses
    /// a kicker it saw kick (or felt kick it) within `kick_memory_s`, or sees in guard stance,
    /// nearer than `clear_lengths` of that kicker's lengths where it senses it, or last sensed
    /// it within its subject's expiry (`intent.subject_lost_s`, or `kick_memory_s` of a kick from
    /// it; [`remembers`]: walking away puts the kicker
    /// behind it, out of sight; [`note_kickers`]), walks straight away from it before anything else — eating, resting, the walk home, a give-up's rest — unless the
    /// kicker is of the lineage it is hunting (the chase's own `dc` rule meets those). A carcass
    /// within that distance of such a kicker is weighed as the hunt weighs a kick
    /// ([`weigh_carcass`]): not worth it, the hound gives it up as a hunt is given up
    /// ([`give_up_carcass`]: shunned for `shun_s`, a rest off for `rest_s`; in an emergency it
    /// waits off it); worth it, it goes, not kept clear of those kickers. Off: as
    /// before (a hound fed, rested and walked home through a guarding mother's kicks). A switch.
    pub keep_clear: bool,
    /// ...the distance it keeps, in the kicker's lengths as it sees them (centre to centre).
    pub clear_lengths: f64,
    /// **Commitment**: a chase starts on a value of at least this.
    pub commit_value: f64,
    /// ...a group (a prey with another of its lineage within `group_m`) is tested first when
    /// its best member is worth at least `test_floor` (below `commit_value`: the test learns
    /// what the group does). Metres, value.
    pub group_m: f64,
    pub test_floor: f64,
    /// **The test**: approach openly to `test_m` of the group's best, watch for
    /// `test_watch_s`, rush for `test_rush_s`; then commit to the best of the group or break
    /// off. Metres, seconds.
    pub test_m: f64,
    pub test_watch_s: f64,
    pub test_rush_s: f64,
    /// A commitment holds its target this long; after it, another prey worth `switch_ratio`
    /// times the target may take the hunt. Seconds, ratio.
    pub commit_s: f64,
    pub switch_ratio: f64,
    /// **Give up**: the value under this; the hound's wound at or over `give_up_wound`; its
    /// stamina spent to `give_up_spent` (1: winded); or `give_up_stall_s` without closing by
    /// `progress_m` or wounding the target. Value, wound, stamina, seconds, metres.
    pub give_up_value: f64,
    pub give_up_wound: f64,
    pub give_up_spent: f64,
    pub give_up_stall_s: f64,
    pub progress_m: f64,
    /// A body given up on (or a group broken off), or a carcass given up to a kicker
    /// (VH-FEED-KICK, [`give_up_carcass`]), is not taken again for this, seconds.
    pub shun_s: f64,
    /// After giving up (a hunt, or a carcass to a kicker) it goes home, or walks `retreat_m` off
    /// the target and rests, for `rest_s` before it hunts again. Seconds, metres.
    pub rest_s: f64,
    pub retreat_m: f64,
    /// **Encircle** (Escobedo et al. 2014): approach to `dc` (a kicker's reach as judged plus
    /// `dc_margin_m`), and inside `da = da_factor × dc` of the target move away from the
    /// packmates and grown kin there. `dc` × `dc_lame_factor` for a lame target, + `dc_kick_m`
    /// after a kick seen from it. Metres, factors.
    pub dc_margin_m: f64,
    pub da_factor: f64,
    pub dc_lame_factor: f64,
    pub dc_kick_m: f64,
    /// ...the approach force reaches full pace `approach_band_m` off `dc` (bounded approach).
    pub approach_band_m: f64,
    /// ...the repulsion's strength, `repel_gain × dc / max(distance, repel_floor × dc)` from
    /// each body (the floor keeps it finite for bodies on top of each other). Factor, × dc.
    pub repel_gain: f64,
    pub repel_floor: f64,
    /// ...and a tangential bias by the hound's approach side, `side_bias × (1 − r / dc)` for
    /// each repelling body nearer than `dc` (a stable per-hound tie-break, so symmetric hounds
    /// part; it fades once they stand apart, so a settled ring does not circle).
    pub side_bias: f64,
    /// ...under this pace (a share of full pace) a hound on the ring stands, facing the
    /// target.
    pub still_pace: f64,
    /// A walk to a point (the test's stand, the cut-off point) has arrived within this,
    /// metres.
    pub arrive_m: f64,
    /// A moving body seen heading within this of straight away from the hound is running
    /// away (`fleeing_gain`), degrees.
    pub flee_deg: f64,
    /// **Cut off**: the point between the target and its nearest grown kin, this share of the
    /// way along the safe part of the line, 0..1; a packmate within `cut_taken_m` of it holds
    /// it already. Fraction, metres.
    pub cut_fraction: f64,
    pub cut_taken_m: f64,
    /// **Intercept**: lead at most `lead_max_s`; a lead that terrain blocks is halved down to
    /// `lead_min_s`, then the interception fails. Seconds.
    pub lead_max_s: f64,
    pub lead_min_s: f64,
    /// ...worth taking when straight pursuit lags: the target crossing the hound's line at
    /// more than `intercept_cross_deg`, or running at `intercept_pace_ratio` of the hound's
    /// sprint or faster. Degrees, ratio.
    pub intercept_cross_deg: f64,
    pub intercept_pace_ratio: f64,
    /// **Harry**: dart in when the target faces another hound within `harry_face_deg`, or
    /// recovers from a kick the hound saw; out again after `harry_s`; only with at least
    /// `harry_hounds` hounds (itself included) within `da` of it. Degrees, seconds, hounds.
    pub harry_face_deg: f64,
    pub harry_s: f64,
    pub harry_hounds: u32,
    /// A packmate it sees moving within `role_heading_deg` of straight at the target, nearer
    /// than it, is driving it (a role read from its movement). Degrees.
    pub role_heading_deg: f64,
    /// **The committed intents' scores** (logits at the lineage's intent temperature): each
    /// option's base, `drive_bonus` per sign the target is driveable (running, young, lame,
    /// isolated), `cut_bonus` per reason to cut off (young or running to adults, a packmate
    /// driving), and `role_damp` off an option a packmate already takes.
    pub w_drive: f64,
    pub drive_bonus: f64,
    pub w_cut_off: f64,
    pub cut_bonus: f64,
    pub w_intercept: f64,
    pub w_encircle: f64,
    pub w_harry: f64,
    pub role_damp: f64,
    /// **Calls** (§2.1, §11.2). Who heeds: a hungry hound with no target heeds a hunt cue
    /// (`heed_cue`) and a regroup howl (`heed_howl`). Switches.
    pub heed_cue: bool,
    pub heed_howl: bool,
    /// ...a hearer adopts the caller's quarry only with its own track on it and the caller in
    /// sight within `cue_caller_m` of where the cue came from, heading at it within
    /// `cue_heading_deg`. Metres, degrees.
    pub cue_caller_m: f64,
    pub cue_heading_deg: f64,
    /// ...it remembers where a regroup howl came from for this long, seconds; a howl from
    /// within `follow_m` of it is not worth walking to, metres.
    pub howl_keep_s: f64,
    pub follow_m: f64,
    /// ...a hungry hound at its den howls at most this often, seconds (0: never).
    pub den_howl_s: f64,
    /// **Strikes** (§11.4): a young body in the capsule whose one-hound `p_kill` is at least
    /// this may be struck in place of the target (an explicit substitution).
    pub substitute_p_kill: f64,
    /// **Strike timing** (VH-FIX): with the target in its capsule, a hound strikes only when
    /// the lunge it would start now lands on the target as it senses it moving (its sighting
    /// carried on at its velocity estimate over the track's age, the windup and the lunge);
    /// else it holds the strike and keeps closing. Off: it strikes whatever is in the capsule
    /// at once (a body fleeing at the lunge's own pace is then never hit). A switch.
    pub time_strikes: bool,
    /// **Closing on a moving target** (VH-FIX): the last metres' approach ([`close_in`]) adds
    /// the target's pace away from the hound, from its own velocity estimate, so it closes to
    /// its stand on a fleeing body instead of trailing it where the approach's slowing matches
    /// the body's pace. Off: the plain approach. A switch.
    pub lead_close: bool,
    /// A carcass within this of where its target was last sensed, when the target is gone
    /// from its senses, is its kill. Metres.
    pub kill_site_m: f64,
    /// **The breeder's den hold** (VH-HOLD; PIT-2's `Pack::whelp_at_den`): a hungry breeder
    /// carrying a litter stays at its den, but leaves to hunt once its own hunger (1 − its
    /// reserve fraction) is at or past this, 0..1 (1: only an emergency releases it; 0: never
    /// held hungry).
    pub whelp_leave_hunger: f64,
    /// ...and leaves with a hunt it senses (a switch): a hunt cue it hears, a hunt already in
    /// hand, or a grown packmate it sees walking away from the den outside the den radius,
    /// heading within `whelp_leave_deg` of straight away from it. Degrees.
    pub whelp_join_hunt: bool,
    pub whelp_leave_deg: f64,
}

impl Default for Tactics {
    fn default() -> Tactics {
        Tactics::VEILHOUND
    }
}

impl Tactics {
    /// The Veilhound's, on (Wrysk's decided behaviour, §0 rows 1, 6 and 8). Placeholders:
    /// - value: meat as mass (cube of length); a metre of chase costs a fiftieth of a meal;
    ///   one hound's chance on an alert, sound adult of its size a quarter, halved again by a
    ///   loftstrider's length (squared);
    /// - the young (under 0.8 of the adult length), the lame, the isolated (6 m from a grown
    ///   kin), the resting and the fleeing raise it; each guard within 6 m takes 0.4 off,
    ///   each grown body facing the pack half (MacNulty et al. 2007; Mech 1970);
    /// - a grown body 1.2 × the hound's length kicks: 0.3 expected kicks, doubled facing a
    ///   hound, at half a meal each (topic 6: three kicks kill);
    /// - VH-FEED-KICK: it keeps two of a kicker's lengths from one that guards or kicked
    ///   within the kick memory (on a hunt, one not of the hunted lineage; centre to centre:
    ///   past a hoof's reach from the front of a body that long, about where a defending
    ///   ungulate's short charge ends);
    /// - groups are tested from 5 m, 3 s watching, a 3 s rush; chases held 30 s; give up at a
    ///   wound of a third, winded, or 20 s without progress; shun 2 min; rest a minute;
    /// - encircle beyond a kick's reach (0.6 of the kicker's length) plus 0.5 m, repulsion
    ///   inside 1.5 × that (Escobedo et al. 2014: `da = 1.5 dc`).
    pub const VEILHOUND: Tactics = Tactics {
        enabled: true,
        meat_exp: 3.0,
        meat_by_mass: true,
        usable_meals: true,
        carcass_keep_s: 3600.0,
        chase_cost_per_m: 0.02,
        p_one: 0.25,
        size_exp: 2.0,
        young_length_fraction: 0.8,
        young_gain: 3.0,
        lame_gain: 2.0,
        isolation_m: 6.0,
        isolation_gain: 1.0,
        resting_gain: 1.0,
        fleeing_gain: 1.0,
        guard_m: 6.0,
        adult_factor: 0.6,
        adults_max: 4,
        facing_factor: 0.5,
        facing_deg: 45.0,
        guard_factor: 0.5,
        kicker_length_ratio: 1.2,
        kick_chance: 0.3,
        kick_facing_gain: 1.0,
        kick_guard_gain: 1.0,
        wound_cost: 0.5,
        hunger_relief: 0.5,
        kick_reach_fraction: 0.6,
        kick_memory_s: 30.0,
        kick_recovery_s: 2.5,
        keep_clear: true,
        clear_lengths: 2.0,
        commit_value: 0.0,
        group_m: 6.0,
        test_floor: -1.0,
        test_m: 5.0,
        test_watch_s: 3.0,
        test_rush_s: 3.0,
        commit_s: 30.0,
        switch_ratio: 1.5,
        give_up_value: 0.0,
        give_up_wound: 0.34,
        give_up_spent: 1.0,
        give_up_stall_s: 20.0,
        progress_m: 0.5,
        shun_s: 120.0,
        rest_s: 60.0,
        retreat_m: 8.0,
        dc_margin_m: 0.5,
        da_factor: 1.5,
        dc_lame_factor: 0.7,
        dc_kick_m: 0.5,
        approach_band_m: 1.0,
        repel_gain: 1.0,
        repel_floor: 0.25,
        side_bias: 0.2,
        still_pace: 0.15,
        arrive_m: 0.25,
        flee_deg: 60.0,
        cut_fraction: 0.3,
        cut_taken_m: 1.5,
        lead_max_s: 4.0,
        lead_min_s: 0.25,
        intercept_cross_deg: 45.0,
        intercept_pace_ratio: 0.8,
        harry_face_deg: 45.0,
        harry_s: 1.5,
        harry_hounds: 2,
        role_heading_deg: 30.0,
        w_drive: 1.0,
        drive_bonus: 0.5,
        w_cut_off: 0.8,
        cut_bonus: 0.5,
        w_intercept: 0.8,
        w_encircle: 1.5,
        w_harry: 2.5,
        role_damp: 1.0,
        heed_cue: true,
        heed_howl: true,
        cue_caller_m: 2.0,
        cue_heading_deg: 30.0,
        howl_keep_s: 120.0,
        follow_m: 3.0,
        den_howl_s: 300.0,
        substitute_p_kill: 0.5,
        time_strikes: true,
        lead_close: true,
        kill_site_m: 2.0,
        // VH-HOLD (placeholders): a breeder holds hungry at the den down to half its reserve,
        // and leaves with a hunt it hears or sees leave (a packmate heading within a right
        // angle of straight away from the den).
        whelp_leave_hunger: 0.5,
        whelp_join_hunt: true,
        whelp_leave_deg: 90.0,
    };
}

// ------------------------------------------------------------------ memory

/// Where a test stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestPhase {
    #[default]
    None,
    /// Walking openly to `test_m` of the group.
    Approach,
    /// Standing, watching.
    Watch,
    /// The short rush at the group.
    Rush,
}

/// **A hound's tactical memory** (§11.5). Saved with the world: a load continues a hunt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Hound {
    /// The body it hunts; committed: a chase, else a test or a stalk.
    pub target: Option<u64>,
    pub committed: bool,
    /// The tick it took the target, and the tick its commitment expires.
    pub since_tick: u64,
    pub until_tick: u64,
    /// The test's phase and the tick that phase began.
    pub test: TestPhase,
    pub phase_tick: u64,
    /// Its own progress: its nearest distance to the target so far and the last tick it
    /// closed by `progress_m` or saw the target's wound rise (it limps).
    pub best_m: f64,
    pub progress_tick: u64,
    /// The target's last two sightings, for its velocity: `[x, z]` and tick; and the estimate.
    pub last: [f64; 2],
    pub last_tick: u64,
    pub vel: [f64; 2],
    /// Bodies it shuns, each until a tick (`(0, 0)`: empty).
    pub shunned: [(u64, u64); 4],
    /// Kicks it saw: `(kicker, tick)`.
    pub kicks: [Option<(u64, u64)>; 3],
    /// The tick it last gave a hunt up (0: never), and the point it walks off from.
    pub gave_up_tick: u64,
    pub gave_up_at: [f64; 2],
    /// A carcass it knows of: its kill, or one it fed at.
    pub carcass: Option<Site>,
    /// Where the last regroup howl it heard came from, and when (`tick` 0: none).
    pub howl: [f64; 2],
    pub howl_tick: u64,
    /// The tick it last howled at its den, and last darted in (harry).
    pub den_howl_tick: u64,
    pub dart_tick: u64,
    /// The target was seen lame at its last sighting (its wound rising is progress).
    pub saw_lame: bool,
    /// Its own hold over the chase's intents (§2.2): the held intent's menu index, the tick
    /// its hold began and the target it was held on.
    pub held: u8,
    pub held_since: u64,
    pub held_on: Option<u64>,
}

impl Hound {
    fn shuns(&self, id: u64, now: u64) -> bool {
        self.shunned.iter().any(|&(s, until)| s == id && now < until)
    }

    fn shun(&mut self, id: u64, until: u64) {
        let k = (0..self.shunned.len()).min_by_key(|&k| if self.shunned[k].0 == id { 0 } else { self.shunned[k].1.max(1) }).unwrap_or(0);
        self.shunned[k] = (id, until);
    }

    fn saw_kick(&mut self, kicker: u64, tick: u64) {
        if let Some((_, at)) = self.kicks.iter_mut().flatten().find(|k| k.0 == kicker) {
            *at = (*at).max(tick);
            return;
        }
        let k = (0..self.kicks.len()).min_by_key(|&k| self.kicks[k].map_or(0, |x| x.1.saturating_add(1))).unwrap_or(0);
        self.kicks[k] = Some((kicker, tick));
    }

    /// The tick it last saw `kicker` kick.
    fn kick_seen(&self, kicker: u64) -> Option<u64> {
        self.kicks.iter().flatten().find(|k| k.0 == kicker).map(|k| k.1)
    }

    fn drop_target(&mut self) {
        self.target = None;
        self.committed = false;
        self.test = TestPhase::None;
    }
}

/// The hound `id`'s memory (default when it has none).
pub(crate) fn memory(fauna: &Fauna, id: u64) -> Hound {
    fauna.hounds.get(&id).copied().unwrap_or_default()
}

/// **The teacher's labels** of a hound's sample (§10.2, §11.3): its proposal (the best intent
/// before the hold), the intent it held, their subject, and the call it made (0 none, `k + 1`
/// the lineage's kind `k`). Session state, read by [`crate::intent::teacher_label`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HoundTeach {
    pub proposal: &'static str,
    pub held: &'static str,
    pub subject: Subject,
    pub call: u8,
}

/// The call kinds' indices ([`crate::calls::kinds_of`]).
const HUNT_CUE: usize = 0;
const REGROUP_HOWL: usize = 1;

// ------------------------------------------------------------------ what it senses

/// The hound itself: what it knows of its own body.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Me {
    pub x: f64,
    pub z: f64,
    pub len: f64,
    pub wound: f64,
    /// 1 − its reserve fraction, 0..1.
    pub hunger: f64,
    pub spent: f64,
    /// Its own body (organic matter) and the meals it can still use from one carcass
    /// ([`Me::usable_meals`]), in units of that body.
    pub body: f64,
    pub usable: f64,
}

impl Me {
    pub(crate) fn of(fauna: &Fauna, a: &Animal) -> Me {
        let len = crate::body_of(&fauna.config, a).map_or(1.0, |g| g.length_m);
        Me {
            x: a.pose.x,
            z: a.pose.z,
            len,
            wound: a.wound.max(0.0),
            hunger: (1.0 - reserve_fraction(fauna, a)).clamp(0.0, 1.0),
            spent: a.founder_state.sprint_spent,
            body: a.body,
            usable: Me::usable_meals(fauna, a),
        }
    }

    /// **The meals a hound can still use from one carcass** (VH-FIX, [`Tactics::usable_meals`]),
    /// in units of its own body, from its own body's state: its stomach's room now, plus what
    /// its gut takes over `carcass_keep_s` (the stomach `k · body` emptied every digestion
    /// time `τ` of carrion), the latter bounded by what it can absorb in that time (its
    /// reserve's room and its upkeep over the keep, at its carrion yield). Infinite when the
    /// switch is off or the body is no founder.
    pub(crate) fn usable_meals(fauna: &Fauna, a: &Animal) -> f64 {
        let Some(f) = a.founder else { return f64::INFINITY };
        let phys = fauna.config.founder(f);
        let Some(tac) = phys.pack.map(|p| p.tactics).filter(|t| t.usable_meals) else { return f64::INFINITY };
        let body = a.body.max(1e-9);
        let core = crate::body::effective_config(&fauna.config, a);
        let carrion = crate::Food::Carrion.index();
        let gut = phys.gut;
        let room = (gut.capacity(body) - a.stomach.organic()).max(0.0) / body;
        let tau = gut.digestion_s[carrion] / gut.throughput_factor(body, core.body_max).max(1e-9);
        let keep = tac.carcass_keep_s.max(0.0);
        let through = if tau > 0.0 { gut.stomach_k * keep / tau } else { f64::INFINITY };
        let yield_ = phys.yield_by_food[carrion].max(1e-9);
        let absorb = ((core.reserve_of(a.body) - a.reserve).max(0.0) + core.maintenance_per_s * body * keep) / yield_ / body;
        room + through.min(absorb)
    }
}

/// The ring delta from `(ax, az)` to `(bx, bz)` and its length.
fn gap(ax: f64, az: f64, bx: f64, bz: f64, ring: f64) -> (f64, f64, f64) {
    let (dx, dz) = crate::pose::wrapped_delta(ax, az, bx, bz, ring);
    (dx, dz, (dx * dx + dz * dz).sqrt())
}

/// `x` around a ring of circumference `ring` (unchanged off the ring: `ring` infinite).
fn on_ring(x: f64, ring: f64) -> f64 {
    if ring.is_finite() && ring > 0.0 { x.rem_euclid(ring) } else { x }
}

/// The smallest angle between two bearings, radians in `[0, π]`.
fn angle_between(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(TAU);
    if d > PI { TAU - d } else { d }
}

/// The adult length of lineage `lineage` (a [`crate::Founder::index`]).
fn adult_len(config: &crate::FaunaConfig, lineage: u8) -> Option<f64> {
    crate::Founder::ALL.get(usize::from(lineage)).map(|&f| config.founder(f).adult_length_m)
}

/// Whether a track looks young: shorter than `young_length_fraction` of its lineage's adult.
pub(crate) fn looks_young(config: &crate::FaunaConfig, tac: &Tactics, t: &Track) -> bool {
    adult_len(config, t.lineage).is_some_and(|l| t.size_m < tac.young_length_fraction * l)
}

/// Whether sight shows body `t` facing the point `(x, z)` within `deg`.
fn faces(t: &Track, x: f64, z: f64, deg: f64, ring: f64) -> bool {
    if t.modes & SIGHT == 0 {
        return false;
    }
    let (dx, dz, d) = gap(t.x, t.z, x, z, ring);
    d > 1e-9 && angle_between(t.heading, dx.atan2(dz)) <= deg.to_radians()
}

/// Whether sight shows body `t` in guard stance (LS-DEFEND: [`crate::perception::stance`]).
fn guarding(t: &Track) -> bool {
    t.modes & SIGHT != 0 && t.stance == crate::perception::stance::GUARD
}

/// Whether body `t` looks like a kicker to a hound of length `own_len`.
fn kicker(tac: &Tactics, t: &Track, own_len: f64) -> bool {
    t.size_m >= tac.kicker_length_ratio * own_len
}

/// The organic body a track of lineage `t.lineage` and length `t.size_m` stands for.
fn body_est(config: &crate::FaunaConfig, t: &Track) -> f64 {
    let Some(&f) = crate::Founder::ALL.get(usize::from(t.lineage)) else { return 0.0 };
    let p = config.founder(f);
    let r = if p.adult_length_m > 0.0 { (t.size_m / p.adult_length_m).clamp(0.0, 1.0) } else { 1.0 };
    p.core.body_max * r * r * r
}

/// The size (window units) a track stands for, and its window size as prey
/// ([`crate::predation::window_size`] on the estimate).
fn sizes_est(config: &crate::FaunaConfig, t: &Track) -> (f64, f64) {
    let Some(&f) = crate::Founder::ALL.get(usize::from(t.lineage)) else { return (0.0, 0.0) };
    let p = config.founder(f);
    let body = body_est(config, t);
    let size = p.size(body);
    let window = if body >= p.core.birth_body { size * p.adult_size_window_factor.max(1.0) } else { size };
    (size, window)
}

/// The kin tracks a hound reads as its grown packmates: own lineage, not young-looking,
/// fresh within the subject's expiry.
fn mates_of(fauna: &Fauna, me: u64, tac: &Tactics, lost_s: f64) -> Vec<Track> {
    let now = fauna.tick;
    fauna.percept(me).map_or_else(Vec::new, |p| {
        p.tracks()
            .iter()
            .filter(|t| t.known && t.class == class::KIN && !looks_young(&fauna.config, tac, t) && age_s(t, now) <= lost_s)
            .copied()
            .collect()
    })
}

/// The prey tracks of its record, fresh within the subject's expiry.
fn prey_of(fauna: &Fauna, me: u64, lost_s: f64) -> Vec<Track> {
    let now = fauna.tick;
    fauna.percept(me).map_or_else(Vec::new, |p| {
        p.tracks().iter().filter(|t| t.known && t.class == class::PREY && age_s(t, now) <= lost_s).copied().collect()
    })
}

fn age_s(t: &Track, now: u64) -> f64 {
    now.saturating_sub(t.tick) as f64 * DT
}

/// **What a hound makes of a prey** (§4.3), from its own senses.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Assess {
    /// The meat counted (at most `usable`), the carcass's whole meat, and what the hounds at
    /// hand can use of it (meals of the hound's body).
    pub meat: f64,
    pub meat_raw: f64,
    pub usable: f64,
    /// One hound's chance, and the chance with the hounds at hand (itself and the packmates
    /// it senses within `pack_radius_m`).
    pub p_one: f64,
    pub p_kill: f64,
    pub chase_cost: f64,
    pub expected_kicks: f64,
    pub risk: f64,
    pub value: f64,
    pub young: bool,
    pub lame: bool,
    /// Distance to its nearest grown kin sensed (∞: none).
    pub kin_m: f64,
    pub guards: u32,
    pub facing: u32,
    pub kicker: bool,
    pub moving: bool,
    pub fleeing: bool,
    /// Standing, seen facing a hound.
    pub at_bay: bool,
    /// Another of its lineage within `group_m`.
    pub in_group: bool,
    pub hounds: u32,
}

/// **Assess prey `t`** for hound `me` (§4.3) from its prey tracks `prey`, its packmates
/// `mates` (grown kin tracks), its kick memory `mem` and pack table `pk` (for
/// `pack_radius_m`). Pure: reads only what the hound senses.
#[allow(clippy::too_many_arguments)]
pub(crate) fn assess(config: &crate::FaunaConfig, tac: &Tactics, pk: &Pack, me: &Me, mem: &Hound, t: &Track, prey: &[Track], mates: &[Track], now: u64, ring: f64) -> Assess {
    let len = t.size_m.max(1e-6);
    let alen = adult_len(config, t.lineage).unwrap_or(len).max(1e-6);
    let young = t.size_m < tac.young_length_fraction * alen;
    let (_, _, d) = gap(me.x, me.z, t.x, t.z, ring);
    // Its grown kin the hound senses, by distance from it.
    let kin: Vec<(&Track, f64)> = prey
        .iter()
        .filter(|k| k.id != t.id && k.lineage == t.lineage && !looks_young(config, tac, k))
        .map(|k| (k, gap(t.x, t.z, k.x, k.z, ring).2))
        .collect();
    let kin_m = kin.iter().map(|k| k.1).fold(f64::INFINITY, f64::min);
    let in_group = prey.iter().any(|k| k.id != t.id && k.lineage == t.lineage && gap(t.x, t.z, k.x, k.z, ring).2 <= tac.group_m);
    let guards: Vec<&Track> = kin.iter().filter(|k| k.1 <= tac.guard_m).map(|k| k.0).collect();
    // Seen facing a hound: itself or a packmate it senses.
    let facing_pack = |k: &Track| faces(k, me.x, me.z, tac.facing_deg, ring) || mates.iter().any(|m| faces(k, m.x, m.z, tac.facing_deg, ring));
    let grown: Vec<&Track> = guards.iter().copied().chain((!young).then_some(t)).collect();
    let facing = grown.iter().filter(|k| facing_pack(k)).count() as u32;
    let guarding_n = guards.iter().filter(|k| guarding(k)).count() as u32;
    let lame = t.lame;
    // Asleep, or still and not seen facing a hound (a still body facing the pack stands at
    // bay: alert, not resting).
    let resting = t.posture == posture::ASLEEP || (t.posture == posture::REST && !facing_pack(t));
    let (mx, mz, md) = gap(me.x, me.z, t.x, t.z, ring);
    let fleeing = t.moving && md > 1e-9 && angle_between(t.heading, mx.atan2(mz)) < tac.flee_deg.to_radians();
    // The factors multiply the odds, `p / (1 − p)`, so none saturates: a calf near the top of
    // the scale is still worth less guarded than alone.
    let base = tac.p_one.clamp(0.0, 1.0 - 1e-9);
    let mut p = base / (1.0 - base) * (me.len / len).powf(tac.size_exp);
    p *= 1.0 + tac.young_gain * (1.0 - len / alen).max(0.0);
    if lame {
        p *= 1.0 + tac.lame_gain;
    }
    if tac.isolation_m > 0.0 {
        p *= 1.0 + tac.isolation_gain * kin_m.min(tac.isolation_m) / tac.isolation_m;
    }
    if resting {
        p *= 1.0 + tac.resting_gain;
    }
    if fleeing {
        p *= 1.0 + tac.fleeing_gain;
    }
    p *= tac.adult_factor.powi((guards.len() as u32).min(tac.adults_max) as i32);
    p *= tac.facing_factor.powi(facing as i32);
    p *= tac.guard_factor.powi(guarding_n as i32);
    let p_one = (p / (1.0 + p)).clamp(0.0, 1.0);
    let hounds = 1 + mates.iter().filter(|m| gap(me.x, me.z, m.x, m.z, ring).2 <= pk.pack_radius_m).count() as u32;
    let p_kill = 1.0 - (1.0 - p_one).powi(hounds as i32);
    // Risk: the kickers about it.
    let expected_kicks: f64 = grown.iter().filter(|k| kicker(tac, k, me.len) && !recovering(tac, mem, k, now)).map(|k| kicks_expected(tac, k, facing_pack(k))).sum();
    let risk = kick_risk(tac, me, expected_kicks);
    let raw = if tac.meat_by_mass { body_est(config, t) / me.body.max(1e-9) } else { (len / me.len.max(1e-6)).powf(tac.meat_exp) };
    // Usable meals (VH-FIX): no more than the hounds at hand can eat before it is gone — its
    // own room taken for each packmate it senses (their guts are not sensed).
    let usable = if tac.usable_meals { me.usable * f64::from(hounds) } else { f64::INFINITY };
    let meat = raw.min(usable);
    let chase_cost = tac.chase_cost_per_m * d;
    let at_bay = !t.moving && facing_pack(t);
    Assess {
        meat,
        meat_raw: raw,
        usable,
        p_one,
        p_kill,
        chase_cost,
        expected_kicks,
        risk,
        value: meat * p_kill - chase_cost - risk,
        young,
        lame,
        kin_m,
        guards: guards.len() as u32,
        facing,
        kicker: !young && kicker(tac, t, me.len),
        moving: t.moving,
        fleeing,
        at_bay,
        in_group,
        hounds,
    }
}

/// Whether kicker `k` is taken as recovering from a kick hound memory `mem` saw (or felt) within
/// `kick_recovery_s`: no kick expected from it.
fn recovering(tac: &Tactics, mem: &Hound, k: &Track, now: u64) -> bool {
    mem.kick_seen(k.id).is_some_and(|at| now.saturating_sub(at) as f64 * DT <= tac.kick_recovery_s)
}

/// The kicks kicker `k` is expected to land: `kick_chance`, × `(1 + kick_facing_gain)` seen
/// `facing` a hound, × `(1 + kick_guard_gain)` in guard stance.
fn kicks_expected(tac: &Tactics, k: &Track, facing: bool) -> f64 {
    tac.kick_chance * if facing { 1.0 + tac.kick_facing_gain } else { 1.0 } * if guarding(k) { 1.0 + tac.kick_guard_gain } else { 1.0 }
}

/// What `expected` kicks cost hound `me`: × `wound_cost`, × `(1 + own wound)`, × `(1 −
/// hunger_relief × hunger)` (a starving hound risks more).
fn kick_risk(tac: &Tactics, me: &Me, expected: f64) -> f64 {
    expected * tac.wound_cost * (1.0 + me.wound) * (1.0 - tac.hunger_relief * me.hunger).clamp(0.0, 1.0)
}

/// The window a hound judges it has (the pack seam, §11.4 split): its own size and its
/// packmates' it senses within `pack_radius_m`, from what it sees of them.
fn sensed_window(fauna: &Fauna, a: &Animal, mates: &[Track], pk: &Pack, ring: f64) -> f64 {
    let own = a.founder.map_or(a.body, |f| fauna.config.founder(f).size(a.body));
    own + mates.iter().filter(|m| gap(a.pose.x, a.pose.z, m.x, m.z, ring).2 <= pk.pack_radius_m).map(|m| sizes_est(&fauna.config, m).0).sum::<f64>()
}

/// Whether the hound at `i` judges it may take prey `t` with window `window`: its list, a
/// young-only list's grown body excluded, the estimated size within its window, and in reach.
fn takes(fauna: &Fauna, i: usize, h: &crate::Hunter, t: &Track, window: f64) -> bool {
    let Some(&pf) = crate::Founder::ALL.get(usize::from(t.lineage)) else { return false };
    if !h.hunts(pf) || !takeable(fauna, i, t) {
        return false;
    }
    let adult = body_est(&fauna.config, t) >= fauna.config.founder(pf).core.birth_body;
    if h.prey_juveniles_only[pf.index()] && adult {
        return false;
    }
    sizes_est(&fauna.config, t).1 <= h.prey_max_ratio * window
}

/// A candidate: its track and assessment.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pick {
    pub track: Track,
    pub assess: Assess,
}

/// **The choice** (§4.3) of hound `i`: of the prey it senses that it judges it takes, not
/// shunned, not in `exclude`, outside every other territory, the best by value (ties: the
/// lower id). Reads only its own record, memory and body ([`assess`]); the caller checks the
/// route ([`reachable`]).
pub(crate) fn choose(fauna: &Fauna, i: usize, exclude: &[u64]) -> Option<Pick> {
    let a = &fauna.animals[i];
    let f = a.founder?;
    let phys = fauna.config.founder(f);
    let pk = phys.pack?;
    let tac = pk.tactics;
    let h = phys.active_hunter()?;
    let lost = phys.intent.subject_lost_s;
    let v = fauna.world_geometry.map_or(1.0, |(_, v)| v);
    let ring = fauna.world_geometry.map_or(f64::INFINITY, |(r, _)| r);
    let me = Me::of(fauna, a);
    let mem = memory(fauna, a.id);
    let prey = prey_of(fauna, a.id, lost);
    let mates = mates_of(fauna, a.id, &tac, lost);
    let window = sensed_window(fauna, a, &mates, &pk, ring);
    let own = a.pack.map(|m| m.pack);
    let mut best: Option<Pick> = None;
    for t in &prey {
        if exclude.contains(&t.id) || mem.shuns(t.id, fauna.tick) || !takes(fauna, i, &h, t, window) {
            continue;
        }
        if own.is_some_and(|own| trespass(fauna, own, t.x, t.z, &pk, v, ring)) {
            continue;
        }
        let s = assess(&fauna.config, &tac, &pk, &me, &mem, t, &prey, &mates, fauna.tick, ring);
        if best.is_some_and(|b| b.assess.value > s.value || (b.assess.value == s.value && b.track.id < t.id)) {
            continue;
        }
        best = Some(Pick { track: *t, assess: s });
    }
    best
}

/// The hound's assessment of body `id` as it senses it now (its record, else its focus's
/// last-known point while fresh).
pub(crate) fn assess_of(fauna: &Fauna, i: usize, id: u64) -> Option<Pick> {
    let a = &fauna.animals[i];
    let f = a.founder?;
    let phys = fauna.config.founder(f);
    let pk = phys.pack?;
    let tac = pk.tactics;
    let lost = phys.intent.subject_lost_s;
    let ring = fauna.world_geometry.map_or(f64::INFINITY, |(r, _)| r);
    let p = fauna.percept(a.id)?;
    let track = p.track(id).copied().or_else(|| Some(p.focus).filter(|t| t.known && t.id == id))?;
    if age_s(&track, fauna.tick) > lost {
        return None;
    }
    let me = Me::of(fauna, a);
    let mem = memory(fauna, a.id);
    let prey = prey_of(fauna, a.id, lost);
    let mates = mates_of(fauna, a.id, &tac, lost);
    Some(Pick { track, assess: assess(&fauna.config, &tac, &pk, &me, &mem, &track, &prey, &mates, fauna.tick, ring) })
}

/// **What a hound makes of each prey it senses now** (a readout: the harness's trace; no rule
/// reads it): `(id, assessment)` for every fresh prey track of hound `id`.
pub(crate) fn values(fauna: &Fauna, id: u64) -> Vec<(u64, Assess)> {
    let Ok(i) = fauna.animals.binary_search_by_key(&id, |a| a.id) else { return Vec::new() };
    let a = &fauna.animals[i];
    let Some(phys) = a.founder.map(|f| fauna.config.founder(f)) else { return Vec::new() };
    prey_of(fauna, id, phys.intent.subject_lost_s).iter().filter_map(|t| assess_of(fauna, i, t.id).map(|p| (t.id, p.assess))).collect()
}

/// Why a hound gives a hunt up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GiveUp {
    /// Its value fell under `give_up_value`.
    Value,
    /// Its own wound passed `give_up_wound`.
    Wound,
    /// Its stamina spent to `give_up_spent`.
    Winded,
    /// `give_up_stall_s` without progress.
    Stalled,
}

impl GiveUp {
    /// The pack event's reason ([`crate::ChaseEnd`]) it is logged under.
    fn chase_end(self) -> crate::ChaseEnd {
        match self {
            GiveUp::Value => crate::ChaseEnd::BrokeOff,
            GiveUp::Wound | GiveUp::Winded => crate::ChaseEnd::Spent,
            GiveUp::Stalled => crate::ChaseEnd::Late,
        }
    }
}

/// **The give-up rule** (§4.3) of a committed hound: its own wound, stamina, time without
/// progress, then the value it puts on the target now. Pure.
pub(crate) fn give_up(tac: &Tactics, me: &Me, value: f64, stalled_s: f64) -> Option<GiveUp> {
    if me.wound >= tac.give_up_wound {
        Some(GiveUp::Wound)
    } else if me.spent >= tac.give_up_spent {
        Some(GiveUp::Winded)
    } else if stalled_s >= tac.give_up_stall_s {
        Some(GiveUp::Stalled)
    } else if value < tac.give_up_value {
        Some(GiveUp::Value)
    } else {
        None
    }
}

// ------------------------------------------------------------------ positioning (pure)

/// The safe distance `dc` from target `t` (assessed `s`) for a hound: a kicker's reach as
/// judged plus `dc_margin_m`, × `dc_lame_factor` lame, + `dc_kick_m` after a kick it saw
/// within `kick_memory_s`; for a body that does not kick, the margin alone.
pub(crate) fn safe_m(tac: &Tactics, t: &Track, s: &Assess, mem: &Hound, now: u64) -> f64 {
    let reach = if s.kicker { tac.kick_reach_fraction * t.size_m } else { 0.0 };
    let mut dc = reach + tac.dc_margin_m;
    if s.lame {
        dc *= tac.dc_lame_factor;
    }
    if mem.kick_seen(t.id).is_some_and(|at| now.saturating_sub(at) as f64 * DT <= tac.kick_memory_s) {
        dc += tac.dc_kick_m;
    }
    dc.max(0.0)
}

/// **Encircle** (Escobedo et al. 2014; §4.2, §7): the velocity, in units of its pace, of a
/// hound at `me` around a target at `t`, with the bodies it repels from (`others`: packmates
/// and the target's grown kin it senses), its approach `side` (±1) and the knobs `tac`. Two
/// forces:
/// - **approach** to `dc`: along the line to the target, `(d − dc) / approach_band_m`
///   clamped to ±1 (it backs off inside `dc`, bounded: never past the target);
/// - **repulsion**, only while it is within `da` of the target, from each other body within
///   `da` of the target: away from it, `repel_gain × dc / max(r, repel_floor × dc)` —
///   packmates close to the prey spread around it. The switch is soft: the repulsion ramps in
///   from nothing at `da` to full at `dc` (`(da − d) / (da − dc)`), so the two forces balance
///   at one distance between `dc` and `da` and a ring settles there instead of chattering
///   across the `da` boundary (a hard switch over a controller period's step does);
/// - and, as a tie-break, a tangential `side × side_bias × (1 − r / dc)` for each repelling
///   body nearer than `dc`, so symmetric hounds part; it is gone once they stand `dc` apart.
pub fn encircle_velocity(me: (f64, f64), t: (f64, f64), others: &[(f64, f64)], dc: f64, da: f64, tac: &Tactics, side: f64, ring: f64) -> (f64, f64) {
    let (dx, dz, d) = gap(me.0, me.1, t.0, t.1, ring);
    if !(d > 1e-9) {
        return (0.0, 0.0);
    }
    let (ux, uz) = (dx / d, dz / d);
    let pull = ((d - dc) / tac.approach_band_m.max(1e-6)).clamp(-1.0, 1.0);
    let (mut vx, mut vz) = (ux * pull, uz * pull);
    if d <= da {
        let on = if da > dc { ((da - d) / (da - dc)).clamp(0.0, 1.0) } else { 1.0 };
        let dc = dc.max(1e-6);
        let mut tangent = 0.0;
        for &(ox, oz) in others {
            if gap(ox, oz, t.0, t.1, ring).2 > da {
                continue;
            }
            let (rx, rz, r) = gap(ox, oz, me.0, me.1, ring);
            if r < dc {
                tangent += 1.0 - r / dc;
            }
            if !(r > 1e-9) {
                continue;
            }
            let k = on * tac.repel_gain * dc / r.max(tac.repel_floor * dc);
            vx += k * rx / r;
            vz += k * rz / r;
        }
        // Tangential: perpendicular to the line to the target.
        vx += side * tac.side_bias * tangent * uz;
        vz -= side * tac.side_bias * tangent * ux;
    }
    (vx, vz)
}

/// **The cut-off point** (§4.2): between target `t` and its nearest defender `d`, at least
/// `t_safe` from the target and `d_safe` from the defender, `fraction` of the way along the
/// safe part of the line; tried there and then at the other safe points along it, the first
/// that `ok` accepts (reachable and safe). `None` when the line has no safe stretch or no
/// point of it is accepted.
pub fn cut_off_point(t: (f64, f64), d: (f64, f64), t_safe: f64, d_safe: f64, fraction: f64, ring: f64, ok: &mut dyn FnMut((f64, f64)) -> bool) -> Option<(f64, f64)> {
    let (dx, dz, l) = gap(t.0, t.1, d.0, d.1, ring);
    let room = l - t_safe - d_safe;
    if !(l > 1e-9) || room < 0.0 {
        return None;
    }
    let (ux, uz) = (dx / l, dz / l);
    let f = fraction.clamp(0.0, 1.0);
    let tries = [f, 0.0, 0.25, 0.5, 0.75, 1.0];
    tries.into_iter().map(|g| t_safe + g * room).find_map(|r| {
        let p = (on_ring(t.0 + ux * r, ring), t.1 + uz * r);
        ok(p).then_some(p)
    })
}

/// **The interception point** (§4.2): where a hound at `me` running at `speed` meets a target
/// at `t` moving at `vel`, within `lead_max_s`; the point must be one the target can reach
/// along its line (`clear`: terrain bounds the prediction) — a blocked lead is halved down
/// to `lead_min_s` (replanned), the point then where the target is at that lead. `None`
/// when no interception is feasible (the target faster and moving away, or every lead
/// blocked): the hound cannot head it off.
#[allow(clippy::too_many_arguments)]
pub fn intercept_point(me: (f64, f64), t: (f64, f64), vel: (f64, f64), speed: f64, lead_max_s: f64, lead_min_s: f64, ring: f64, clear: &mut dyn FnMut((f64, f64)) -> bool) -> Option<((f64, f64), f64)> {
    let (rx, rz, _) = gap(me.0, me.1, t.0, t.1, ring);
    // |r + v t| = s t: (v·v − s²) t² + 2 (r·v) t + r·r = 0.
    let a = vel.0 * vel.0 + vel.1 * vel.1 - speed * speed;
    let b = 2.0 * (rx * vel.0 + rz * vel.1);
    let c = rx * rx + rz * rz;
    let lead = if a.abs() < 1e-12 {
        if b < 0.0 { Some(-c / b) } else { None }
    } else {
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 {
            None
        } else {
            let q = disc.sqrt();
            [(-b - q) / (2.0 * a), (-b + q) / (2.0 * a)].into_iter().filter(|x| *x > 0.0).fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.min(x))))
        }
    }?;
    if !(lead <= lead_max_s) {
        return None;
    }
    let mut l = lead;
    while l >= lead_min_s.max(1e-6) {
        let p = (on_ring(t.0 + vel.0 * l, ring), t.1 + vel.1 * l);
        if clear(p) {
            return Some((p, l));
        }
        l *= 0.5;
    }
    None
}

/// The hound's approach side, ±1: a stable per-hound tie-break (§11.5).
pub(crate) fn side_of(id: u64) -> f64 {
    if id.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 63 == 0 { 1.0 } else { -1.0 }
}

// ------------------------------------------------------------------ calls

/// **Kicks it sees** (§11.5): a prey it has in sight this sensing whose kick winds up or
/// recovers is remembered with the tick (the act is visible; nothing else of that body is
/// read).
fn observe_kicks(fauna: &mut Fauna, me: u64) {
    let now = fauna.tick;
    let Some(p) = fauna.percept(me) else { return };
    let seen: Vec<u64> = p
        .tracks()
        .iter()
        .filter(|t| t.known && t.class == class::PREY && t.modes & SIGHT != 0 && t.tick == p.tick)
        .filter(|t| {
            fauna.animals.binary_search_by_key(&t.id, |o| o.id).ok().is_some_and(|j| fauna.animals[j].kick.phase != crate::kick::Phase::Ready)
        })
        .map(|t| t.id)
        .collect();
    if seen.is_empty() {
        return;
    }
    let m = fauna.hounds.entry(me).or_default();
    for id in seen {
        m.saw_kick(id, now);
    }
}

/// **A kick felt** (VH-FEED-KICK): a kick that landed on pack body `target` is remembered as
/// a kick from `kicker` it saw — its own body tells it whose hoof it was, whether or not its
/// eyes were on the kicker (a feeding hound's head is down). With [`Tactics::keep_clear`]
/// off, nothing (as before).
pub(crate) fn felt_kick(fauna: &mut Fauna, target: u64, kicker: u64) {
    let now = fauna.tick;
    let Ok(j) = fauna.animals.binary_search_by_key(&target, |a| a.id) else { return };
    let a = &fauna.animals[j];
    let on = a.pack.is_some() && a.founder.and_then(|f| fauna.config.founder(f).pack).is_some_and(|pk| pk.tactics.enabled && pk.tactics.keep_clear);
    if on {
        fauna.hounds.entry(target).or_default().saw_kick(kicker, now);
    }
}

/// Whether hound `a` (of length `len`, its memory `mem`) keeps clear of body `t` as it senses
/// or remembers it: a kicker (grown, at least `kicker_length_ratio` of its length) it saw or
/// felt kick within `kick_memory_s`, or one shown in guard stance.
fn clears(fauna: &Fauna, tac: &Tactics, t: &Track, len: f64, mem: &Hound, now: u64) -> bool {
    if !t.known || t.class != class::PREY || !kicker(tac, t, len) || looks_young(&fauna.config, tac, t) {
        return false;
    }
    let kicked = mem.kick_seen(t.id).is_some_and(|at| now.saturating_sub(at) as f64 * DT <= tac.kick_memory_s);
    kicked || guarding(t)
}

/// **The kickers it remembers** (VH-FEED-KICK, [`crate::Fauna::kickers_seen`]) for hound `i`,
/// each sample before [`keep_clear`]: every kicker it senses now that it keeps clear of
/// ([`clears`]) is remembered as sensed (one it senses that no longer qualifies is forgotten),
/// and one not sensed for its subject's expiry (`subject_lost_s`) is forgotten unless its kick
/// is still remembered ([`remembers`]). Walking away
/// from a kicker turns its back on it: a still kicker behind it is neither seen nor heard,
/// and with only what it sensed this sample the hound stopped, turned back to its carcass
/// and ate under the hoof (s7: kicked three times, never moving). Off with the switch.
pub(crate) fn note_kickers(fauna: &mut Fauna, i: usize, tac: &Tactics) {
    let a = fauna.animals[i];
    if !tac.keep_clear {
        fauna.kickers_seen.remove(&a.id);
        return;
    }
    let Some(f) = a.founder else { return };
    let lost = fauna.config.founder(f).intent.subject_lost_s;
    let now = fauna.tick;
    let len = Me::of(fauna, &a).len;
    let mem = memory(fauna, a.id);
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    let sensed: Vec<Track> = fauna
        .percept(a.id)
        .map(|p| p.tracks().iter().filter(|t| t.known && t.class == class::PREY && now.saturating_sub(t.tick) <= fresh).copied().collect())
        .unwrap_or_default();
    let mut kept: Vec<Track> = fauna.kickers_seen.get(&a.id).cloned().unwrap_or_default();
    kept.retain(|k| remembers(tac, &mem, k, now, lost) && !sensed.iter().any(|t| t.id == k.id));
    kept.extend(sensed.iter().filter(|t| clears(fauna, tac, t, len, &mem, now)).copied());
    if kept.is_empty() {
        fauna.kickers_seen.remove(&a.id);
    } else {
        fauna.kickers_seen.insert(a.id, kept);
    }
}

/// Whether a hound with memory `mem` still remembers where kicker `k` stood, unsensed since:
/// within its subject's expiry `lost` of last sensing it, or (follow-up 2) while it remembers
/// a kick from it (`kick_memory_s`). Kicked, walked clear and forgetting where the hoof stood
/// after `subject_lost_s` while still remembering the kick, it walked back to its carcass by
/// the family and was kicked again (s7, m203 save: h104, 13 s after its first kick).
fn remembers(tac: &Tactics, mem: &Hound, k: &Track, now: u64, lost: f64) -> bool {
    age_s(k, now) <= lost || mem.kick_seen(k.id).is_some_and(|at| now.saturating_sub(at) as f64 * DT <= tac.kick_memory_s)
}

/// The kickers hound `i` keeps clear of ([`clears`]) as it senses them now, then those it
/// remembers ([`note_kickers`], [`remembers`]) and does not sense now.
fn kickers_kept(fauna: &Fauna, i: usize, tac: &Tactics) -> Vec<Track> {
    let a = &fauna.animals[i];
    let now = fauna.tick;
    let len = Me::of(fauna, a).len;
    let mem = memory(fauna, a.id);
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    let sensed = fauna.percept(a.id).map_or(&[][..], |p| p.tracks());
    let sensed = sensed.iter().filter(|t| now.saturating_sub(t.tick) <= fresh);
    let lost = a.founder.map_or(0.0, |f| fauna.config.founder(f).intent.subject_lost_s);
    let remembered = fauna.kickers_seen.get(&a.id).map_or(&[][..], |v| v.as_slice());
    let remembered = remembered.iter().filter(|k| remembers(tac, &mem, k, now, lost) && !fauna.percept(a.id).is_some_and(|p| p.tracks().iter().any(|t| t.id == k.id && now.saturating_sub(t.tick) <= fresh)));
    sensed.chain(remembered).filter(|t| clears(fauna, tac, t, len, &mem, now)).copied().collect()
}

/// **Keep clear of a kicker** (VH-FEED-KICK, [`Tactics::keep_clear`]) for hound `i`, from its
/// own record and its memory of the kickers it last sensed ([`note_kickers`]): those it
/// keeps clear of ([`clears`]) standing nearer than `clear_lengths` of their own length,
/// where it senses them now or last sensed them — except those of lineage `hunting`, the one
/// its hunt is on. `Some`: walk straight away from them (each weighted by how far inside its
/// distance the hound stands), feeding off, with the nearest as the subject.
pub(crate) fn keep_clear(fauna: &Fauna, i: usize, tac: &Tactics, hunting: Option<u8>, ring: f64) -> Option<(Actions, Track)> {
    keep_clear_but(fauna, i, tac, hunting, ring, &[])
}

/// [`keep_clear`] but for the kickers `dared`: those over a carcass the hound judges worth
/// their kicks ([`weigh_carcass`]).
fn keep_clear_but(fauna: &Fauna, i: usize, tac: &Tactics, hunting: Option<u8>, ring: f64, dared: &[u64]) -> Option<(Actions, Track)> {
    if !tac.keep_clear {
        return None;
    }
    let a = &fauna.animals[i];
    let (mut sx, mut sz) = (0.0, 0.0);
    let mut nearest: Option<(f64, Track)> = None;
    for t in kickers_kept(fauna, i, tac) {
        if Some(t.lineage) == hunting || dared.contains(&t.id) {
            continue;
        }
        let keep = tac.clear_lengths * t.size_m;
        let (dx, dz, d) = gap(a.pose.x, a.pose.z, t.x, t.z, ring);
        if !(d < keep) {
            continue;
        }
        let w = (keep - d) / keep.max(1e-9);
        if d > 1e-9 {
            sx -= dx / d * w;
            sz -= dz / d * w;
        }
        if nearest.is_none_or(|(nd, n)| d < nd || (d == nd && t.id < n.id)) {
            nearest = Some((d, t));
        }
    }
    let (_, k) = nearest?;
    let away = if sx * sx + sz * sz > 1e-18 {
        sx.atan2(sz)
    } else {
        // On top of it (or exactly between two): turn from the way it faces.
        a.pose.heading_rad + PI
    };
    Some((head(a, away, 1.0, 0.0), k))
}

/// **A carcass under a kicker, weighed** (VH-FEED-KICK follow-up 2): what hound `i` makes of
/// carcass `s` (carrion `carrion` there) with kickers it keeps clear of ([`kickers_kept`])
/// standing within `clear_lengths` of their lengths of it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Weighed {
    /// The kickers over it, and the one nearest the hound (its subject while it waits).
    pub over: Vec<u64>,
    pub kicker: Track,
    /// Its value: the meals it can use of it, less the walk to it, less the kicks' cost.
    pub value: f64,
    /// Worth the kicks: it goes (and is not kept clear of those kickers).
    pub dare: bool,
}

/// **Weigh a carcass under a kicker the way the hunt weighs a kick** (VH-FEED-KICK follow-up 2,
/// with [`Tactics::keep_clear`]). `None`: no kicker it keeps clear of stands within
/// `clear_lengths` of its lengths of carcass `s`. Else its value as [`assess`] values a prey
/// that is already dead (`p_kill` 1): the meals it can use (`carrion` over its own body, at
/// most its usable meals), less `chase_cost_per_m` of the walk to it, less the risk of the
/// kickers over it ([`kicks_expected`]; one recovering from a kick counts too, unlike the hunt's
/// strike, as a meal outlasts the recovery), costed by [`kick_risk`]: a starving hound risks
/// more, a wounded one less. It dares on a
/// value of at least `commit_value` when the hunt would not give up on it ([`give_up`]: its
/// wound, winded, or the value under `give_up_value`). Otherwise it waits it out (wolves wait
/// off a carcass a mother guards): kicked and walked clear, it was walked straight back to
/// eat beside the family and kicked dead on the return (s7, m203 save: h104, h102).
pub(crate) fn weigh_carcass(fauna: &Fauna, i: usize, tac: &Tactics, s: Site, carrion: f64, v: f64, ring: f64) -> Option<Weighed> {
    if !tac.keep_clear {
        return None;
    }
    let (cx, cz) = centre(s, v);
    let over: Vec<Track> = kickers_kept(fauna, i, tac).into_iter().filter(|t| gap(t.x, t.z, cx, cz, ring).2 < tac.clear_lengths * t.size_m).collect();
    if over.is_empty() {
        return None;
    }
    let a = &fauna.animals[i];
    let me = Me::of(fauna, a);
    let lost = a.founder.map_or(0.0, |f| fauna.config.founder(f).intent.subject_lost_s);
    let mates = mates_of(fauna, a.id, tac, lost);
    let facing_pack = |k: &Track| faces(k, me.x, me.z, tac.facing_deg, ring) || mates.iter().any(|m| faces(k, m.x, m.z, tac.facing_deg, ring));
    // Every kicker over it counts, one recovering from a kick too: a meal outlasts a kick's
    // recovery (the hunt's exception is for a strike's moment).
    let expected: f64 = over.iter().map(|k| kicks_expected(tac, k, facing_pack(k))).sum();
    let risk = kick_risk(tac, &me, expected);
    let meat = (carrion.max(0.0) / me.body.max(1e-9)).min(me.usable);
    let value = meat - tac.chase_cost_per_m * gap(me.x, me.z, cx, cz, ring).2 - risk;
    let dare = value >= tac.commit_value && give_up(tac, &me, value, 0.0).is_none();
    let kicker = *over
        .iter()
        .map(|t| (gap(me.x, me.z, t.x, t.z, ring).2, t))
        .min_by(|p, q| p.0.total_cmp(&q.0).then(p.1.id.cmp(&q.1.id)))
        .map(|(_, t)| t)
        .expect("not empty");
    Some(Weighed { over: over.iter().map(|t| t.id).collect(), kicker, value, dare })
}

/// Hound `i` holds off carcass `s`: it weighs it ([`weigh_carcass`]) and does not dare. The
/// kicker it waits on.
fn holds_off(fauna: &Fauna, i: usize, tac: &Tactics, fv: &FloraView<'_>, s: Site, v: f64, ring: f64) -> Option<Track> {
    let carrion = fv.ground_at(s).map_or(0.0, |g| g.carrion);
    weigh_carcass(fauna, i, tac, s, carrion, v, ring).filter(|w| !w.dare).map(|w| w.kicker)
}

/// Waiting off carcass `s`: standing, turned to watch it (and the kickers over it).
fn wait_off(a: &Animal, s: Site, v: f64, ring: f64) -> Actions {
    let (x, z) = centre(s, v);
    let (dx, dz, d) = gap(a.pose.x, a.pose.z, x, z, ring);
    if d > 1e-9 { head(a, dx.atan2(dz), 0.0, 0.0) } else { Actions::REST }
}

/// The tag of a carcass in a hound's shun list ([`Hound::shunned`], whose entries are body
/// ids): a carcass is no body, so its site is packed under the top bit, which no body id
/// reaches (VH-FEED-KICK follow-up 3; no saved field added, no schema bump).
const CARCASS_KEY: u64 = 1 << 63;

/// Carcass `s` as an entry of a hound's shun list ([`CARCASS_KEY`]; 21 bits a coordinate).
fn carcass_key(s: Site) -> u64 {
    const M: u64 = (1 << 21) - 1;
    CARCASS_KEY | ((u64::from(s.x) & M) << 42) | ((u64::from(s.y) & M) << 21) | (u64::from(s.z) & M)
}

/// Hound `a` (memory `mem`) shuns carcass `s` now: it gave it up to a kicker within `shun_s`
/// ([`give_up_carcass`]). Never in an emergency (its hunger past its reserve's floor): then it
/// weighs the carcass again ([`weigh_carcass`]).
fn shuns_carcass(a: &Animal, mem: &Hound, s: Site, now: u64) -> bool {
    !a.emergency && mem.shuns(carcass_key(s), now)
}

/// **A carcass given up to a kicker** (VH-FEED-KICK follow-up 3), as a hunt is given up
/// ([`start_give_up`]): hound `i`, judging carcass `s` not worth the kicks of the kickers over
/// it ([`weigh_carcass`]), shuns it for `shun_s` (it keeps it in mind and weighs it again after;
/// not in an emergency, [`shuns_carcass`]) and rests off it for `rest_s` ([`rest_off`]: clear of
/// the kicker first, then home or `retreat_m` off the carcass), labelled a give-up on kicker
/// `k`. Before this the hound stood waiting at the kicker's kept distance and walked straight
/// back the moment it lost the kicker from its senses and its kick from memory: kicked dead on
/// the return (s7, m203 save: h102 at 204.92, 31 s after its last kick).
#[allow(clippy::too_many_arguments)]
fn give_up_carcass(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, tac: &Tactics, pk: &Pack, h: &crate::Hunter, geometry: &crate::Body, den: Site, s: Site, k: &Track) -> Actions {
    let a = fauna.animals[i];
    let now = fauna.tick;
    let v = view.config.voxel_m;
    let mut mem = memory(fauna, a.id);
    mem.shun(carcass_key(s), now + cubarium_voxel::ticks_at_least_one(tac.shun_s));
    mem.gave_up_tick = now;
    let (cx, cz) = centre(s, v);
    mem.gave_up_at = [cx, cz];
    fauna.hounds.insert(a.id, mem);
    label(fauna, a.id, "give_up", Subject::of_track(k));
    rest_off(fauna, i, view, tac, pk, h, geometry, mem, den)
}

/// **A hunt cue heard** (§11.2): the freshest one's point, if any.
fn cue_heard(fauna: &Fauna, me: u64, keep_s: f64) -> Option<crate::calls::CallRecord> {
    let now = fauna.tick;
    crate::calls::heard(fauna, me)
        .iter()
        .filter(|r| usize::from(r.kind) == HUNT_CUE && now.saturating_sub(r.tick) as f64 * DT <= keep_s)
        .max_by_key(|r| r.tick)
        .copied()
}

/// **Adopting a caller's quarry** (§2.1, §11.2): from hunt cue `r`, the prey hound `me`
/// adopts — one it has its own track on, with a packmate in sight within `cue_caller_m` of
/// where the cue came from heading at it within `cue_heading_deg`. The record gives only the
/// cue's bearing and point; never the quarry. Of several, the nearest the caller.
pub(crate) fn adopt_from_cue(fauna: &Fauna, me: u64, r: &crate::calls::CallRecord, tac: &Tactics, lost_s: f64, ring: f64) -> Option<u64> {
    let prey = prey_of(fauna, me, lost_s);
    let callers: Vec<Track> = mates_of(fauna, me, tac, lost_s)
        .into_iter()
        .filter(|m| m.modes & SIGHT != 0 && gap(m.x, m.z, r.x, r.z, ring).2 <= tac.cue_caller_m)
        .collect();
    let mut best: Option<(f64, u64)> = None;
    for c in &callers {
        for t in &prey {
            let (dx, dz, d) = gap(c.x, c.z, t.x, t.z, ring);
            if d > 1e-9 && angle_between(c.heading, dx.atan2(dz)) <= tac.cue_heading_deg.to_radians() && best.is_none_or(|b| d < b.0) {
                best = Some((d, t.id));
            }
        }
    }
    best.map(|b| b.1)
}

/// Emit call kind `kind` from hound `i`; the call for its teacher's label (`k + 1`), else 0.
fn call_kind(fauna: &mut Fauna, i: usize, kind: usize) -> u8 {
    if crate::calls::emit(fauna, i, kind) { kind as u8 + 1 } else { 0 }
}

// ------------------------------------------------------------------ bookkeeping

/// **Bookkeeping, a chase begins** (census and harness readouts; read by no rule): the pack's
/// record names the target and logs [`crate::PackEvent::ChaseStart`] when it is new to it.
fn bookkeep_start(fauna: &mut Fauna, i: usize, own: u64, pick: &Pick) {
    let a = fauna.animals[i];
    let now = fauna.tick;
    let prev = fauna.pack(own).and_then(|p| p.target);
    if prev != Some(pick.track.id) {
        if let Some(p) = fauna.pack_mut(own) {
            p.target = Some(pick.track.id);
            p.chase_since_tick = now;
            p.target_site = Some(pick.track.face());
        }
        fauna.ledger.pack_chases += 1;
        for (k, bit) in crate::perception::MODALITIES.iter().enumerate() {
            if pick.track.modes & bit != 0 {
                fauna.ledger.pack_chases_by_sense[k] += 1;
            }
        }
        fauna.log_pack_event(crate::PackEvent::ChaseStart {
            tick: now,
            pack: own,
            participants: pick.assess.hounds,
            taker: a.id,
            target: pick.track.id,
            score: pick.assess.value,
            prev,
            prev_score: None,
        });
    }
    crate::perception::approach(fauna, a.id, pick.track.id);
}

/// **Bookkeeping, a hound drops `tid`** (`why`): logged on the pack's record when no other
/// hound of the pack still hunts it (their memories, read for the readout only).
fn bookkeep_end(fauna: &mut Fauna, i: usize, own: u64, tid: u64, why: crate::ChaseEnd) {
    let me = fauna.animals[i].id;
    let still = fauna.animals.iter().any(|b| b.id != me && b.pack.is_some_and(|m| m.pack == own) && fauna.hounds.get(&b.id).is_some_and(|h| h.target == Some(tid)));
    if still {
        return;
    }
    let now = fauna.tick;
    match why {
        crate::ChaseEnd::BrokeOff => fauna.ledger.pack_break_offs += 1,
        crate::ChaseEnd::Lost => fauna.ledger.pack_targets_lost += 1,
        crate::ChaseEnd::Killed => fauna.ledger.pack_chase_kills += 1,
        _ => {}
    }
    if let Some(p) = fauna.pack_mut(own) {
        if p.target == Some(tid) {
            p.target = None;
            p.target_site = None;
        }
        if !matches!(why, crate::ChaseEnd::Killed | crate::ChaseEnd::Gone) {
            p.abandoned = Some(tid);
        }
    }
    fauna.log_pack_event(crate::PackEvent::ChaseEnd { tick: now, pack: own, why, target: tid, by: me });
}

/// **Bookkeeping, a kill** at `site`: the pack's feast record for the census and its pups'
/// provisioning readout (read by no rule of this tactic).
fn bookkeep_feast(fauna: &mut Fauna, own: u64, site: Site) {
    let now = fauna.tick;
    let old = fauna.pack(own).and_then(|p| p.feast).filter(|&f| f != site);
    if let Some(old) = old {
        let eaten = fauna.pack(own).map_or(0.0, |p| p.feast_eaten);
        fauna.log_pack_event(crate::PackEvent::FeastEnd { tick: now, pack: own, site: old, eaten, why: crate::FeastEnd::Replaced });
    }
    if let Some(p) = fauna.pack_mut(own) {
        if p.feast != Some(site) {
            p.feast_eaten = 0.0;
        }
        p.feast = Some(site);
    }
}

// ------------------------------------------------------------------ walking

/// Walk hound `a` toward the point `(gx, gz)` on a face at layer `y`: straight when its route
/// graph walks the line (or either end is off it), else the next node of the route to it
/// (searched out to the chase's reach). `forward` scales a straight walk (a sprint above 1);
/// stops within `stop_m`.
#[allow(clippy::too_many_arguments)]
fn walk_to(fauna: &mut Fauna, view: &VoxelView<'_>, founder: crate::Founder, a: &Animal, geometry: &crate::Body, h: &crate::Hunter, pk: &Pack, (gx, gz): (f64, f64), y: u32, forward: f64, stop_m: f64, feed: f64) -> Actions {
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let (dx, dz, d) = delta(a, gx, gz, ring);
    if d <= stop_m {
        return Actions { forward: 0.0, turn: 0.0, feed };
    }
    let straight = |a: &Animal| head(a, dx.atan2(dz), forward, feed);
    let (own_r, _) = node_radii(geometry, h, v);
    let graph = fauna.pack_routes(view, founder);
    let col = |m: f64, n: u32| ((m / v).floor() as i64).rem_euclid(i64::from(n.max(1))) as u32;
    let goal = Site { x: col(gx, view.config.width), y, z: ((gz / v).floor().max(0.0) as u32).min(view.config.depth.saturating_sub(1)) };
    let (Some(from), Some(to)) = (graph.node_near(a.site, (a.pose.x, a.pose.z), own_r), graph.node_near(goal, (gx, gz), own_r + 1)) else {
        return straight(a);
    };
    if from == to || graph.walk_line_to(from, (a.pose.x, a.pose.z), (gx, gz), to) {
        return straight(a);
    }
    let cruise = fauna.config.manifest(founder).cruise_m_per_s;
    let field = graph.distances(to, reach_steps(pk, cruise, v));
    match route_point(fauna, founder, &graph, &field, from, a) {
        Some(p) => route_go(fauna, view, a, founder, p, ring, feed),
        None => straight(a),
    }
}

/// Stand facing the point `(x, z)`, feed `feed`.
fn face_point(a: &Animal, x: f64, z: f64, ring: f64, feed: f64) -> Actions {
    let (dx, dz, _) = delta(a, x, z, ring);
    let diff = angle_to(a.pose.heading_rad, dx.atan2(dz));
    Actions { forward: 0.0, turn: (diff / std::f64::consts::FRAC_PI_4).clamp(-1.0, 1.0), feed }
}

// ------------------------------------------------------------------ the tactic

/// The Veilhound menu's index of intent `name`.
fn ix(name: &str) -> usize {
    VEILHOUND.index(name).expect("a Veilhound intent")
}

/// Label the hound's sample: its proposal, held intent, subject and call.
fn teach(fauna: &mut Fauna, id: u64, proposal: &'static str, held: &'static str, subject: Subject, call: u8) {
    let call = fauna.hound_teach.get(&id).map_or(call, |t| if call == 0 { t.call } else { call });
    fauna.hound_teach.insert(id, HoundTeach { proposal, held, subject, call });
    hint(fauna, id, held);
}

/// One label for both (no hold between them: a phase or a body state).
fn label(fauna: &mut Fauna, id: u64, name: &'static str, subject: Subject) {
    teach(fauna, id, name, name, subject, 0);
}

/// **The Veilhound's tactic** (module note) for pack body `i`, over its own heuristic's
/// `held` actions. `None` leaves them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn tactic(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    clock_on: bool,
    daylight: f64,
    held: Actions,
    occupancy: Option<crate::senses::Occ<'_>>,
) -> Option<Actions> {
    let a = fauna.animals[i];
    let m = a.pack?;
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let pk = phys.pack?;
    let tac = pk.tactics;
    let h = phys.active_hunter()?;
    let den = fauna.pack(m.pack)?.den;
    let own = m.pack;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let (dcx, dcz) = centre(den, v);
    let d_den = distance(&a, dcx, dcz, ring);
    let geometry = phys.body_at(a.body);
    let now = fauna.tick;
    let id = a.id;
    fauna.hound_teach.remove(&id);
    let feast_left = |s: Site| fv.ground_at(s).is_some_and(|g| g.carrion > 0.0 && g.carcass.is_some());
    let at_feast = |fs: Site| {
        let cols = crate::body::mouth_columns(view, &a.pose, &geometry);
        crate::body::mouth_pool_site(fv, &cols, a.site.y, crate::Food::Carrion).is_some_and(|(s, _)| s == fs)
    };
    let eat = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };
    let in_den = |s: Site| {
        let (x, z) = centre(s, v);
        let (dx, dz) = crate::pose::wrapped_delta(dcx, dcz, x, z, ring);
        (dx * dx + dz * dz).sqrt() <= pk.den_radius_m
    };

    // A pup: at a carcass it senses within the den radius when hungry, else at the den.
    if m.pup {
        label(fauna, id, "den", Subject::None);
        if a.eating {
            crate::perception::refresh(fauna, i, view, fv, None, crate::perception::Want { carcasses: true, ..Default::default() });
            let near = fauna.percept(id).and_then(|p| p.carcass_sites().filter(|&s| in_den(s) && feast_left(s)).min_by(|p, q| {
                let (px, pz) = centre(*p, v);
                let (qx, qz) = centre(*q, v);
                distance(&a, px, pz, ring).total_cmp(&distance(&a, qx, qz, ring)).then(p.cmp(q))
            }));
            if let Some(fs) = near {
                label(fauna, id, "feed", Subject::None);
                if at_feast(fs) {
                    return Some(eat);
                }
                let (fx, fz) = centre(fs, v);
                return Some(go_to(&a, fx, fz, ring, 0.0, 1.0));
            }
        }
        if d_den > 0.5 * pk.den_radius_m {
            let (to, routed) = way_home(fauna, view, founder, &a, &geometry, &h, den);
            return Some(if routed { route_go(fauna, view, &a, founder, to, ring, 0.0) } else { go_to(&a, to.0, to.1, ring, 0.0, 0.0) });
        }
        return Some(Actions { forward: 0.0, turn: held.turn, feed: held.feed });
    }

    crate::perception::refresh(fauna, i, view, fv, occupancy, crate::perception::Want { scent: true, carcasses: true, ..Default::default() });
    let manifest = fauna.config.manifest(founder);
    let guard = manifest.cruise_m_per_s * manifest.controller_period_s + (h.strike_reach_length_fraction + h.jaw_forward_length_fraction) * geometry.length_m;
    let clamp = |fauna: &Fauna, act: Actions| keep_out(fauna, i, own, act, &pk, guard, v, ring);
    let lost_s = phys.intent.subject_lost_s;
    observe_kicks(fauna, id);
    // A regroup howl it hears is remembered with where it came from.
    if let Some(r) = crate::calls::heard(fauna, id).iter().filter(|r| usize::from(r.kind) == REGROUP_HOWL).max_by_key(|r| r.tick).copied() {
        let mm = fauna.hounds.entry(id).or_default();
        if r.tick > mm.howl_tick {
            mm.howl = [r.x, r.z];
            mm.howl_tick = r.tick;
        }
    }
    let hungry = a.eating || a.emergency;

    // VH-FEED-KICK: a kicker that guards or kicked it, near: away first — off the carcass,
    // out of the rest or the walk home; on a hunt, from any kicker but the hunted lineage's.
    note_kickers(fauna, i, &tac);
    let hunting = memory(fauna, id).target.and_then(|t| fauna.percept(id).and_then(|p| p.track(t)).map(|k| k.lineage));
    // Follow-up 2: a hungry hound off a hunt that judges the carcass it would feed at worth the
    // kicks of the kickers over it ([`weigh_carcass`]) goes, and is not kept clear of them.
    // Follow-up 3: a carcass it gave up to a kicker is shunned for `shun_s` ([`shuns_carcass`]).
    let (feed_site, weighed): (Option<Site>, Option<Weighed>) = {
        let mem = memory(fauna, id);
        let resting_off = mem.gave_up_tick > 0 && now.saturating_sub(mem.gave_up_tick) as f64 * DT < tac.rest_s;
        let feeds = a.eating && (a.emergency || !wants_rest(fauna, i, clock_on, daylight)) && !mem.committed && !resting_off;
        let scan = fauna.config.hunt.carcass_scan_m.max(0.0);
        let site = feeds.then(|| mem.carcass.filter(|&s| feast_left(s) && !shuns_carcass(&a, &mem, s, now))).flatten().or_else(|| {
            feeds.then(|| fauna.percept(id).and_then(|p| {
                p.carcass_sites()
                    .filter(|&s| feast_left(s) && !shuns_carcass(&a, &mem, s, now))
                    .map(|s| {
                        let (x, z) = centre(s, v);
                        (distance(&a, x, z, ring), s)
                    })
                    .filter(|&(d, _)| d <= scan)
                    .min_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)))
                    .map(|(_, s)| s)
            }))
            .flatten()
        });
        (site, site.and_then(|s| weigh_carcass(fauna, i, &tac, s, fv.ground_at(s).map_or(0.0, |g| g.carrion), v, ring)))
    };
    let dared: Vec<u64> = weighed.as_ref().filter(|w| w.dare).map(|w| w.over.clone()).unwrap_or_default();
    if let Some((act, k)) = keep_clear_but(fauna, i, &tac, hunting, ring, &dared) {
        // Follow-up 3: walked off the carcass it would feed at by a kicker over it that it
        // judges not worth the kicks: the carcass is given up (shunned, rest off).
        if let (Some(s), Some(w)) = (feed_site, weighed.as_ref().filter(|w| !w.dare && !a.emergency)) {
            let act = give_up_carcass(fauna, i, view, &tac, &pk, &h, &geometry, den, s, &w.kicker);
            return Some(clamp(fauna, act));
        }
        label(fauna, id, "give_up", Subject::of_track(&k));
        return Some(clamp(fauna, act));
    }

    // Fed or out of phase: home to rest; a hunt in hand is dropped.
    if !a.emergency && (wants_rest(fauna, i, clock_on, daylight) || !a.eating) {
        drop_hunt(fauna, i, own, crate::ChaseEnd::Spent);
        if must_walk_home(fauna, i, view) {
            let (to, routed) = way_home(fauna, view, founder, &a, &geometry, &h, den);
            if routed {
                label(fauna, id, "den", Subject::None);
                return Some(clamp(fauna, route_go(fauna, view, &a, founder, to, ring, 0.0)));
            }
        }
        label(fauna, id, "rest", Subject::None);
        return Some(Actions::REST);
    }

    // The breeder carrying a litter stays home (PIT-2), eating a carcass it knows by the den;
    // VH-HOLD: unless its own hunger or a hunt it senses releases it ([`whelp_leaves`]).
    if pk.whelp_at_den
        && fauna.pack(own).is_some_and(|p| p.breeder == Some(id))
        && a.reproduction.escrow.is_some()
        && !a.emergency
        && !whelp_leaves(fauna, &a, &tac, phys.calls.keep_s, lost_s, (dcx, dcz, pk.den_radius_m), ring)
    {
        drop_hunt(fauna, i, own, crate::ChaseEnd::Spent);
        label(fauna, id, "den", Subject::None);
        if a.eating {
            let mem = memory(fauna, id);
            let mut sites: Vec<(f64, Site)> = mem
                .carcass
                .into_iter()
                .chain(fauna.percept(id).map(|p| p.carcass_sites().collect::<Vec<_>>()).unwrap_or_default())
                .filter(|&s| in_den(s) && feast_left(s) && !shuns_carcass(&a, &mem, s, now))
                .map(|s| {
                    let (x, z) = centre(s, v);
                    (distance(&a, x, z, ring), s)
                })
                .collect();
            sites.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)));
            // Follow-up 2: one under a kicker not worth its kicks is left out; with only
            // those, it gives the nearest up (follow-up 3), or in an emergency waits off it.
            let held_off: Vec<(Site, Track)> = sites.iter().filter_map(|&(_, s)| holds_off(fauna, i, &tac, fv, s, v, ring).map(|k| (s, k))).collect();
            if !sites.is_empty() && held_off.len() == sites.len() {
                let (s, k) = held_off[0];
                if !a.emergency {
                    let act = give_up_carcass(fauna, i, view, &tac, &pk, &h, &geometry, den, s, &k);
                    return Some(clamp(fauna, act));
                }
                label(fauna, id, "give_up", Subject::of_track(&k));
                return Some(clamp(fauna, wait_off(&a, s, v, ring)));
            }
            sites.retain(|(_, s)| !held_off.iter().any(|(h, _)| h == s));
            if let Some(&(_, s)) = sites.first() {
                label(fauna, id, "feed", Subject::None);
                if at_feast(s) {
                    return Some(clamp(fauna, eat));
                }
                if fauna.config.hunt.meal_first {
                    if let Some(go) = crate::hunt::meal_step_to(fauna, i, view, fv, s) {
                        return Some(clamp(fauna, go));
                    }
                } else {
                    let (x, z) = centre(s, v);
                    return Some(clamp(fauna, go_to(&a, x, z, ring, 0.0, 1.0)));
                }
            }
        }
        if d_den > 0.5 * pk.den_radius_m {
            let (to, routed) = way_home(fauna, view, founder, &a, &geometry, &h, den);
            let go = if routed { route_go(fauna, view, &a, founder, to, ring, 0.0) } else { go_to(&a, to.0, to.1, ring, 0.0, 0.0) };
            return Some(clamp(fauna, go));
        }
        return Some(Actions::REST);
    }
    if hungry {
        fauna.ledger.pack_hungry_samples += 1;
    }

    // ---- the hunt in hand: killed, lost, given up.
    let mut mem = memory(fauna, id);
    if let Some(tid) = mem.target {
        let sensed = fauna.percept(id).and_then(|p| p.track(tid).copied());
        let last = fauna.percept(id).map(|p| p.focus).filter(|f| f.known && f.id == tid);
        let lastk = sensed.or(last);
        // Gone from its senses with a carcass where it was last sensed: its kill.
        let kill = if sensed.is_none() {
            lastk.and_then(|k| {
                fauna.percept(id).and_then(|p| p.carcass_sites().filter(|&s| feast_left(s)).find(|&s| {
                    let (x, z) = centre(s, v);
                    gap(k.x, k.z, x, z, ring).2 <= tac.kill_site_m
                }))
            })
        } else {
            None
        };
        if let Some(site) = kill {
            mem.drop_target();
            mem.carcass = Some(site);
            fauna.hounds.insert(id, mem);
            bookkeep_end(fauna, i, own, tid, crate::ChaseEnd::Killed);
            bookkeep_feast(fauna, own, site);
        } else if lastk.is_none_or(|k| age_s(&k, now) > lost_s) {
            mem.drop_target();
            mem.shun(tid, now + cubarium_voxel::ticks_at_least_one(tac.shun_s));
            fauna.hounds.insert(id, mem);
            bookkeep_end(fauna, i, own, tid, crate::ChaseEnd::Lost);
        } else if let Some(k) = lastk {
            // Progress and the velocity estimate, from its own sightings: closing by
            // `progress_m` past its nearest so far (a breakaway out of contact restarts the
            // measure), or the target newly limping.
            let d = distance(&a, k.x, k.z, ring);
            if mem.best_m <= pk.close_in_m && d > pk.close_in_m {
                mem.best_m = d;
            }
            if d <= mem.best_m - tac.progress_m || (k.lame && !mem.saw_lame) {
                mem.best_m = d;
                mem.progress_tick = now;
            }
            mem.saw_lame = k.lame;
            if k.tick > mem.last_tick && mem.last_tick > 0 {
                let dt = (k.tick - mem.last_tick) as f64 * DT;
                let (dx, dz, _) = gap(mem.last[0], mem.last[1], k.x, k.z, ring);
                if dt > 0.0 {
                    mem.vel = [dx / dt, dz / dt];
                }
            }
            if k.tick > mem.last_tick {
                mem.last = [k.x, k.z];
                mem.last_tick = k.tick;
            }
            if !k.moving {
                mem.vel = [0.0, 0.0];
            }
            fauna.hounds.insert(id, mem);
            fauna.percepts.entry(id).or_default().set_focus(tid, now);
            if mem.committed {
                let s = assess_of(fauna, i, tid).map_or(f64::NEG_INFINITY, |p| p.assess.value);
                let stalled = now.saturating_sub(mem.progress_tick) as f64 * DT;
                if let Some(why) = give_up(&tac, &Me::of(fauna, &a), s, stalled) {
                    let act = start_give_up(fauna, i, view, own, &tac, &pk, &h, &geometry, k, why);
                    return Some(clamp(fauna, act));
                }
            }
        }
    }
    let mut mem = memory(fauna, id);

    // ---- given up: home or walk off and rest, for `rest_s`.
    if mem.gave_up_tick > 0 && now.saturating_sub(mem.gave_up_tick) as f64 * DT < tac.rest_s {
        let act = rest_off(fauna, i, view, &tac, &pk, &h, &geometry, mem, den);
        label(fauna, id, "give_up", Subject::None);
        return Some(clamp(fauna, act));
    }

    // ---- a carcass it knows: feed (and howl the pack to it once).
    if !mem.committed {
        let known = mem.carcass.filter(|&s| feast_left(s));
        if mem.carcass.is_some() && known.is_none() {
            mem.carcass = None;
            fauna.hounds.insert(id, mem);
        }
        // Follow-up 3: one it gave up to a kicker it keeps in mind, shunned for `shun_s`.
        if let Some(s) = known.filter(|&s| !shuns_carcass(&a, &mem, s, now)) {
            // Follow-up 2: a carcass under a kicker not worth its kicks: given up (follow-up
            // 3), or in an emergency waited out off it.
            if let Some(k) = holds_off(fauna, i, &tac, fv, s, v, ring) {
                if !a.emergency {
                    let act = give_up_carcass(fauna, i, view, &tac, &pk, &h, &geometry, den, s, &k);
                    return Some(clamp(fauna, act));
                }
                label(fauna, id, "give_up", Subject::of_track(&k));
                return Some(clamp(fauna, wait_off(&a, s, v, ring)));
            }
            if at_feast(s) {
                let call = call_kind(fauna, i, REGROUP_HOWL);
                teach(fauna, id, "feed", "feed", Subject::None, call);
                return Some(clamp(fauna, eat));
            }
            if let Some(go) = if fauna.config.hunt.meal_first { crate::hunt::meal_step_to(fauna, i, view, fv, s) } else {
                let (x, z) = centre(s, v);
                Some(go_to(&a, x, z, ring, 0.0, 1.0))
            } {
                label(fauna, id, "feed", Subject::None);
                return Some(clamp(fauna, go));
            }
        }
        if mem.target.is_none() {
            // Follow-up 2: the carcasses it senses under a kicker not worth its kicks are left
            // out; with only those, it gives the nearest up (follow-up 3; in an emergency it
            // waits off it). Those it shuns (follow-up 3) are left out.
            let scan = fauna.config.hunt.carcass_scan_m.max(0.0);
            let mut sensed: Vec<(f64, Site)> = fauna
                .percept(id)
                .map(|p| {
                    p.carcass_sites()
                        .filter(|&s| feast_left(s))
                        .map(|s| {
                            let (x, z) = centre(s, v);
                            (distance(&a, x, z, ring), s)
                        })
                        .filter(|&(d, _)| d <= scan)
                        .collect()
                })
                .unwrap_or_default();
            sensed.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)));
            let shunned = sensed.iter().any(|&(_, s)| shuns_carcass(&a, &mem, s, now));
            sensed.retain(|&(_, s)| !shuns_carcass(&a, &mem, s, now));
            let held_off: Vec<(Site, Track)> = sensed.iter().filter_map(|&(_, s)| holds_off(fauna, i, &tac, fv, s, v, ring).map(|k| (s, k))).collect();
            let step = if held_off.is_empty() && !shunned {
                crate::hunt::meal_step(fauna, i, view, fv)
            } else if let Some(&(_, s)) = sensed.iter().find(|(_, s)| !held_off.iter().any(|(h, _)| h == s)) {
                crate::hunt::meal_step_to(fauna, i, view, fv, s)
            } else if let Some(&(s, k)) = held_off.first() {
                if !a.emergency {
                    let act = give_up_carcass(fauna, i, view, &tac, &pk, &h, &geometry, den, s, &k);
                    return Some(clamp(fauna, act));
                }
                label(fauna, id, "give_up", Subject::of_track(&k));
                return Some(clamp(fauna, wait_off(&a, s, v, ring)));
            } else {
                None
            };
            if let Some(go) = step {
                // The carcass it fed at becomes one it knows.
                let site = crate::body::mouth_pool_site(fv, &crate::body::mouth_columns(view, &a.pose, &geometry), a.site.y, crate::Food::Carrion).map(|(s, _)| s);
                if let Some(s) = site.filter(|&s| feast_left(s)) {
                    fauna.hounds.entry(id).or_default().carcass = Some(s);
                }
                label(fauna, id, "feed", Subject::None);
                return Some(clamp(fauna, go));
            }
        }
    }

    // ---- a hunt cue it hears: adopt the caller's quarry, with its own track on it.
    if mem.target.is_none() && tac.heed_cue {
        let keep = phys.calls.keep_s;
        if let Some(r) = cue_heard(fauna, id, keep) {
            if let Some(q) = adopt_from_cue(fauna, id, &r, &tac, lost_s, ring) {
                if let Some(p) = assess_of(fauna, i, q).filter(|p| !mem.shuns(q, now) && p.assess.value >= tac.test_floor) {
                    commit(fauna, i, own, &p, &tac);
                    mem = memory(fauna, id);
                }
            }
        }
    }

    // ---- a new target: the best it senses.
    if mem.target.is_none() {
        // The best it senses whose strike face it can walk to within the chase.
        let mut no: Vec<u64> = Vec::new();
        let mut pick = None;
        while let Some(p) = choose(fauna, i, &no) {
            if reachable(fauna, view, founder, &pk, &geometry, &h, &a, &p.track) {
                pick = Some(p);
                break;
            }
            no.push(p.track.id);
            if no.len() >= 4 {
                break;
            }
        }
        if let Some(p) = pick {
            if p.assess.in_group && p.assess.value >= tac.test_floor {
                // A group: test it.
                fauna.ledger.hound_tests += 1;
                let mm = fauna.hounds.entry(id).or_default();
                mm.target = Some(p.track.id);
                mm.committed = false;
                mm.test = TestPhase::Approach;
                mm.phase_tick = now;
                mm.since_tick = now;
                mm.best_m = distance(&a, p.track.x, p.track.z, ring);
                mm.progress_tick = now;
                mm.last = [p.track.x, p.track.z];
                mm.last_tick = p.track.tick;
                fauna.percepts.entry(id).or_default().set_focus(p.track.id, now);
            } else if !p.assess.in_group && p.assess.value >= tac.commit_value {
                // A lone prey worth it: stalk it (committed: it is the hunt).
                commit(fauna, i, own, &p, &tac);
            }
        }
        mem = memory(fauna, id);
    }

    // ---- the test.
    if mem.target.is_some() && !mem.committed {
        let act = test_step(fauna, i, view, own, &tac, &pk, &h, &geometry, mem);
        return Some(clamp(fauna, act));
    }

    // ---- the chase: score the committed intents, hold one, run its skill.
    if let (Some(tid), true) = (mem.target, mem.committed) {
        // After its commitment expires, a prey worth `switch_ratio` times it may take the hunt.
        if now >= mem.until_tick {
            let cur = assess_of(fauna, i, tid).map_or(f64::NEG_INFINITY, |p| p.assess.value);
            if let Some(q) = choose(fauna, i, &[tid]).filter(|q| q.assess.value >= tac.switch_ratio * cur.max(1e-9) && q.assess.value >= tac.commit_value) {
                let old = tid;
                fauna.hounds.entry(id).or_default().drop_target();
                bookkeep_end(fauna, i, own, old, crate::ChaseEnd::Switched);
                commit(fauna, i, own, &q, &tac);
            } else {
                fauna.hounds.entry(id).or_default().until_tick = now + cubarium_voxel::ticks_at_least_one(tac.commit_s);
            }
            mem = memory(fauna, id);
        }
        if let Some(act) = chase(fauna, i, view, own, &tac, &pk, &h, &geometry, mem) {
            return Some(clamp(fauna, act));
        }
    }

    // ---- search.
    let act = search(fauna, i, view, fv, &tac, &pk, &h, &geometry, held, occupancy, den);
    Some(clamp(fauna, act))
}

/// **VH-HOLD, the breeder's release** from its den hold (hungry, carrying a litter, not in an
/// emergency): its own hunger at or past `whelp_leave_hunger`; or, with `whelp_join_hunt`, a
/// hunt it senses — one in hand (its own target: it joined a hunt and keeps it), a hunt cue it
/// heard within `keep_s`, or a grown packmate it sees (fresh within `lost_s`) moving outside
/// the den radius and heading within `whelp_leave_deg` of straight away from the den (§2.1:
/// a visible departure). Reads only its own body, memory, hearing and sight, never the pack's
/// record or a packmate's hunger.
pub(crate) fn whelp_leaves(fauna: &Fauna, a: &Animal, tac: &Tactics, keep_s: f64, lost_s: f64, (dcx, dcz, den_r): (f64, f64, f64), ring: f64) -> bool {
    if 1.0 - reserve_fraction(fauna, a) >= tac.whelp_leave_hunger {
        return true;
    }
    if !tac.whelp_join_hunt {
        return false;
    }
    if memory(fauna, a.id).target.is_some() || cue_heard(fauna, a.id, keep_s).is_some() {
        return true;
    }
    mates_of(fauna, a.id, tac, lost_s).iter().any(|m| {
        let (dx, dz, d) = gap(dcx, dcz, m.x, m.z, ring);
        m.modes & SIGHT != 0 && m.moving && d > den_r && angle_between(m.heading, dx.atan2(dz)) <= tac.whelp_leave_deg.to_radians()
    })
}

/// Whether hound `a` can reach prey `t` on foot within the chase (the strike node on its
/// route field): the old choice's bound (bellwing-finding item 3).
#[allow(clippy::too_many_arguments)]
fn reachable(fauna: &mut Fauna, view: &VoxelView<'_>, founder: crate::Founder, pk: &Pack, geometry: &crate::Body, h: &crate::Hunter, a: &Animal, t: &Track) -> bool {
    let routes = fauna.pack_routes(view, founder);
    reachable_on(fauna, &routes, view.config.voxel_m, founder, pk, geometry, h, a, t)
}

/// [`reachable`] on `routes`, read only.
#[allow(clippy::too_many_arguments)]
fn reachable_on(fauna: &Fauna, routes: &crate::encounter::RouteGraph, v: f64, founder: crate::Founder, pk: &Pack, geometry: &crate::Body, h: &crate::Hunter, a: &Animal, t: &Track) -> bool {
    let (own_r, _) = node_radii(geometry, h, v);
    let Some(from) = routes.node_near(a.site, (a.pose.x, a.pose.z), own_r) else { return true };
    let cruise = fauna.config.manifest(founder).cruise_m_per_s;
    let field = routes.distances(from, reach_steps(pk, cruise, v));
    strike_node(routes, t, height_of(fauna, t.id), geometry, h, v).is_some_and(|j| field.contains_key(&j))
}

/// VH-TEACH: **the Veilhound's `Reach` block** for hound `i` and the prey on its `Prey` slot
/// (`prey`, as the slot shows it: known and within `fresh_s`):
/// - `den`: it knows a walking way home ([`super::den_way`]);
/// - `prey`: it can walk to a face it would strike that prey from within a chase
///   ([`reachable`], the choice's own bound), on its lineage's route graph as last built;
/// - `prey_lost`: it lost that prey within `shun_s` (its own memory of the chase,
///   [`Hound::shuns`]).
///
/// The tactic chooses only prey it can reach and has not just lost, and walks a sated hound
/// home only along a way it knows; without these a brain could not tell its search with such
/// a prey beside it from a hunt, or its rest away from a den it cannot reach from the walk
/// home. Read only; no slot prey reads 0 on both prey channels. A brain body's lost chases
/// are its teacher's memory (the tactic writes it), so only a heuristic or dry-taught hound
/// fills `prey_lost`.
pub(crate) fn reach(fauna: &Fauna, i: usize, view: &VoxelView<'_>, prey: &Track, fresh_s: f64) -> [f64; 3] {
    let a = &fauna.animals[i];
    let den = f64::from(super::den_way(fauna, a, view));
    let Some(founder) = a.founder else { return [den, 0.0, 0.0] };
    let phys = fauna.config.founder(founder);
    let slot = prey.known && fresh_s > 0.0 && prey.age(fauna.tick) as f64 * DT <= fresh_s;
    let (Some(pk), Some(h), true) = (phys.pack, phys.active_hunter(), slot) else { return [den, 0.0, 0.0] };
    let geometry = phys.body_at(a.body);
    let way = fauna
        .pack_routes
        .get(&founder.index())
        .is_some_and(|(_, routes)| reachable_on(fauna, routes, view.config.voxel_m, founder, &pk, &geometry, &h, a, prey));
    [den, f64::from(way), f64::from(memory(fauna, a.id).shuns(prey.id, fauna.tick))]
}

/// **Commit** hound `i` to `p` (a chase): its memory, focus, the hunt cue, and bookkeeping.
fn commit(fauna: &mut Fauna, i: usize, own: u64, p: &Pick, tac: &Tactics) {
    let a = fauna.animals[i];
    let now = fauna.tick;
    let ring = fauna.world_geometry.map_or(f64::INFINITY, |(r, _)| r);
    let mm = fauna.hounds.entry(a.id).or_default();
    let fresh = mm.target != Some(p.track.id);
    mm.target = Some(p.track.id);
    mm.committed = true;
    mm.test = TestPhase::None;
    mm.since_tick = now;
    mm.until_tick = now + cubarium_voxel::ticks_at_least_one(tac.commit_s);
    mm.progress_tick = now;
    mm.best_m = gap(a.pose.x, a.pose.z, p.track.x, p.track.z, ring).2;
    mm.saw_lame = p.track.lame;
    if fresh {
        mm.last = [p.track.x, p.track.z];
        mm.last_tick = p.track.tick;
        mm.vel = [0.0, 0.0];
    }
    fauna.percepts.entry(a.id).or_default().set_focus(p.track.id, now);
    let call = call_kind(fauna, i, HUNT_CUE);
    if call > 0 {
        fauna.hound_teach.entry(a.id).or_default().call = call;
    }
    bookkeep_start(fauna, i, own, p);
}

/// Drop hound `i`'s hunt (fed, resting, whelping): bookkeeping with `why`.
fn drop_hunt(fauna: &mut Fauna, i: usize, own: u64, why: crate::ChaseEnd) {
    let id = fauna.animals[i].id;
    let mut mem = memory(fauna, id);
    let Some(tid) = mem.target else { return };
    let committed = mem.committed;
    mem.drop_target();
    fauna.hounds.insert(id, mem);
    if committed {
        bookkeep_end(fauna, i, own, tid, why);
    }
}

/// **Give up** (§4.3): break off, shun the target, the regroup howl, rest; bookkeeping.
#[allow(clippy::too_many_arguments)]
fn start_give_up(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, own: u64, tac: &Tactics, pk: &Pack, h: &crate::Hunter, geometry: &crate::Body, k: Track, why: GiveUp) -> Actions {
    let a = fauna.animals[i];
    let now = fauna.tick;
    let mut mem = memory(fauna, a.id);
    let Some(tid) = mem.target else { return Actions::REST };
    mem.drop_target();
    mem.shun(tid, now + cubarium_voxel::ticks_at_least_one(tac.shun_s));
    mem.gave_up_tick = now;
    mem.gave_up_at = [k.x, k.z];
    fauna.hounds.insert(a.id, mem);
    fauna.ledger.hound_give_ups += 1;
    bookkeep_end(fauna, i, own, tid, why.chase_end());
    let call = call_kind(fauna, i, REGROUP_HOWL);
    teach(fauna, a.id, "give_up", "give_up", Subject::of_track(&k), call);
    let den = fauna.pack(own).map(|p| p.den).unwrap_or(a.site);
    rest_off(fauna, i, view, tac, pk, h, geometry, mem, den)
}

/// After giving up: home when it can walk there (its den walk), else off the target to
/// `retreat_m`, then rest.
#[allow(clippy::too_many_arguments)]
fn rest_off(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, tac: &Tactics, _pk: &Pack, h: &crate::Hunter, geometry: &crate::Body, mem: Hound, den: Site) -> Actions {
    let a = fauna.animals[i];
    let Some(founder) = a.founder else { return Actions::REST };
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    // VH-FEED-KICK: the kicker that ended the hunt first (the hunt is dropped: every kicker).
    if let Some((act, _)) = keep_clear(fauna, i, tac, None, ring) {
        return act;
    }
    if must_walk_home(fauna, i, view) {
        let (to, routed) = way_home(fauna, view, founder, &a, geometry, h, den);
        if routed {
            return route_go(fauna, view, &a, founder, to, ring, 0.0);
        }
        return go_to(&a, to.0, to.1, ring, 0.0, 0.0);
    }
    let (dx, dz, d) = delta(&a, mem.gave_up_at[0], mem.gave_up_at[1], ring);
    if d < tac.retreat_m && d > 1e-9 {
        return head(&a, (-dx).atan2(-dz), 1.0, 0.0);
    }
    Actions::REST
}

/// **The test** (§4.2, §4.3): approach openly to `test_m`, watch, a short rush; at its end,
/// or as soon as one of the group bolts worth a chase, commit to the best of the group, else
/// break off (the group shunned, the howl, rest).
#[allow(clippy::too_many_arguments)]
fn test_step(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, own: u64, tac: &Tactics, pk: &Pack, h: &crate::Hunter, geometry: &crate::Body, mut mem: Hound) -> Actions {
    let a = fauna.animals[i];
    let founder = a.founder.expect("a pack lineage");
    let now = fauna.tick;
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let Some(p) = mem.target.and_then(|tid| assess_of(fauna, i, tid)) else {
        fauna.hounds.entry(a.id).or_default().drop_target();
        return Actions { forward: 0.0, turn: 0.0, feed: 0.0 };
    };
    let t = p.track;
    // The group: the target and its kin it senses within `group_m`.
    let lost = fauna.config.founder(founder).intent.subject_lost_s;
    let group: Vec<Track> = prey_of(fauna, a.id, lost).into_iter().filter(|k| k.lineage == t.lineage && gap(t.x, t.z, k.x, k.z, ring).2 <= tac.group_m).collect();
    let best = group.iter().filter_map(|k| assess_of(fauna, i, k.id)).filter(|q| !mem.shuns(q.track.id, now)).fold(None::<Pick>, |b, q| match b {
        Some(b) if b.assess.value >= q.assess.value => Some(b),
        _ => Some(q),
    });
    // One that bolts and is worth it is taken at once (selection by pursuit).
    if let Some(q) = best.filter(|q| q.assess.value >= tac.commit_value && (q.assess.fleeing || !q.assess.in_group)) {
        fauna.ledger.hound_test_commits += 1;
        commit(fauna, i, own, &q, tac);
        return chase(fauna, i, view, own, tac, pk, h, geometry, memory(fauna, a.id)).unwrap_or(Actions { forward: 0.0, turn: 0.0, feed: 1.0 });
    }
    let since = now.saturating_sub(mem.phase_tick) as f64 * DT;
    let d = distance(&a, t.x, t.z, ring);
    let subject = Subject::of_track(&t);
    match mem.test {
        TestPhase::None | TestPhase::Approach => {
            if d <= tac.test_m {
                mem.test = TestPhase::Watch;
                mem.phase_tick = now;
                fauna.hounds.insert(a.id, mem);
                label(fauna, a.id, "test", subject);
                return face_point(&a, t.x, t.z, ring, 0.0);
            }
            mem.test = TestPhase::Approach;
            fauna.hounds.insert(a.id, mem);
            label(fauna, a.id, "test", subject);
            // Openly, to `test_m` short of it, from its own side.
            let (dx, dz, dd) = gap(t.x, t.z, a.pose.x, a.pose.z, ring);
            let (ux, uz) = if dd > 1e-9 { (dx / dd, dz / dd) } else { (0.0, 1.0) };
            let goal = (t.x + ux * tac.test_m, t.z + uz * tac.test_m);
            walk_to(fauna, view, founder, &a, geometry, h, pk, goal, t.site.1, 1.0, tac.arrive_m, 0.0)
        }
        TestPhase::Watch => {
            label(fauna, a.id, "test", subject);
            if since >= tac.test_watch_s {
                mem.test = TestPhase::Rush;
                mem.phase_tick = now;
                fauna.hounds.insert(a.id, mem);
                let call = call_kind(fauna, i, HUNT_CUE);
                teach(fauna, a.id, "test", "test", subject, call);
            }
            face_point(&a, t.x, t.z, ring, 0.0)
        }
        TestPhase::Rush => {
            if since >= tac.test_rush_s {
                match best.filter(|q| q.assess.value >= tac.commit_value) {
                    Some(q) => {
                        fauna.ledger.hound_test_commits += 1;
                        commit(fauna, i, own, &q, tac);
                        return chase(fauna, i, view, own, tac, pk, h, geometry, memory(fauna, a.id)).unwrap_or(Actions { forward: 0.0, turn: 0.0, feed: 1.0 });
                    }
                    None => {
                        // The break-off: the group shunned, the howl, rest.
                        let until = now + cubarium_voxel::ticks_at_least_one(tac.shun_s);
                        let tid = t.id;
                        mem.drop_target();
                        for k in &group {
                            mem.shun(k.id, until);
                        }
                        mem.shun(tid, until);
                        mem.gave_up_tick = now;
                        mem.gave_up_at = [t.x, t.z];
                        fauna.hounds.insert(a.id, mem);
                        fauna.ledger.hound_give_ups += 1;
                        bookkeep_end(fauna, i, own, tid, crate::ChaseEnd::BrokeOff);
                        let call = call_kind(fauna, i, REGROUP_HOWL);
                        teach(fauna, a.id, "test", "give_up", subject, call);
                        let den = fauna.pack(own).map(|p| p.den).unwrap_or(a.site);
                        return rest_off(fauna, i, view, tac, pk, h, geometry, mem, den);
                    }
                }
            }
            label(fauna, a.id, "test", subject);
            let top = crate::gait::sprint_forward(&fauna.config, founder);
            if let Some(act) = close_in(fauna, view, founder, &a, &t, geometry, h, pk, ring) {
                return timed(fauna, tac, &a, geometry, h, &t, &mem, ring, act);
            }
            let act = walk_to(fauna, view, founder, &a, geometry, h, pk, (t.x, t.z), t.site.1, top, 0.0, 1.0);
            timed(fauna, tac, &a, geometry, h, &t, &mem, ring, act)
        }
    }
}

/// The committed options' scores (logits; `−∞` masked) for hound `i` on target `p`, with the
/// points its skills would take: the cut-off point and the interception point.
struct Options {
    scores: Vec<f64>,
    cut: Option<(f64, f64)>,
    intercept: Option<(f64, f64)>,
    dc: f64,
    da: f64,
    others: Vec<(f64, f64)>,
    opening: bool,
}

#[allow(clippy::too_many_arguments)]
fn options(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, tac: &Tactics, pk: &Pack, h: &crate::Hunter, geometry: &crate::Body, mem: &Hound, p: &Pick) -> Options {
    let a = fauna.animals[i];
    let founder = a.founder.expect("a pack lineage");
    let now = fauna.tick;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let lost = fauna.config.founder(founder).intent.subject_lost_s;
    let t = p.track;
    let s = p.assess;
    let mates = mates_of(fauna, a.id, tac, lost);
    let prey = prey_of(fauna, a.id, lost);
    let kin: Vec<Track> = prey.iter().filter(|k| k.id != t.id && k.lineage == t.lineage && !looks_young(&fauna.config, tac, k)).copied().collect();
    let mut sc = vec![f64::NEG_INFINITY; VEILHOUND.len()];
    let dc = safe_m(tac, &t, &s, mem, now);
    let da = tac.da_factor * dc;
    let (_, _, d) = gap(a.pose.x, a.pose.z, t.x, t.z, ring);
    // Roles read from packmates' movement: driving (moving at it, nearer than this hound).
    let driving = mates.iter().any(|m| {
        let (mx, mz, md) = gap(m.x, m.z, t.x, t.z, ring);
        m.moving && md < d && md > 1e-9 && angle_between(m.heading, mx.atan2(mz)) <= tac.role_heading_deg.to_radians()
    });
    let signs = f64::from(u8::from(s.fleeing) + u8::from(s.young) + u8::from(s.lame) + u8::from(s.kin_m > tac.isolation_m));
    sc[ix("drive")] = tac.w_drive + tac.drive_bonus * signs - if driving { tac.role_damp } else { 0.0 };
    // Cut off: between it and its nearest grown kin, beyond each one's reach as judged.
    let nearest = kin.iter().min_by(|x, y| gap(t.x, t.z, x.x, x.z, ring).2.total_cmp(&gap(t.x, t.z, y.x, y.z, ring).2));
    let mut cut = None;
    if let Some(dk) = nearest {
        let me = Me::of(fauna, &a);
        let mem2 = *mem;
        let dk_assess = assess(&fauna.config, tac, pk, &me, &mem2, dk, &prey, &mates, now, ring);
        let d_safe = safe_m(tac, dk, &dk_assess, &mem2, now);
        let others: Vec<(Track, f64)> = kin.iter().map(|k| {
            let ka = assess(&fauna.config, tac, pk, &me, &mem2, k, &prey, &mates, now, ring);
            (*k, safe_m(tac, k, &ka, &mem2, now))
        }).collect();
        let (own_r, _) = node_radii(geometry, h, v);
        let graph = fauna.pack_routes(view, founder);
        let from = graph.node_near(a.site, (a.pose.x, a.pose.z), own_r);
        let cruise = fauna.config.manifest(founder).cruise_m_per_s;
        let field = from.map(|f| graph.distances(f, reach_steps(pk, cruise, v)));
        let width = view.config.width;
        let depth = view.config.depth;
        let mut ok = |q: (f64, f64)| {
            // Safe from every grown kin it senses, and reachable on its route graph.
            if others.iter().any(|(k, sm)| gap(k.x, k.z, q.0, q.1, ring).2 < *sm) {
                return false;
            }
            let (Some(field), Some(_)) = (field.as_ref(), from) else { return true };
            let site = Site { x: ((q.0 / v).floor() as i64).rem_euclid(i64::from(width)) as u32, y: t.site.1, z: ((q.1 / v).floor().max(0.0) as u32).min(depth.saturating_sub(1)) };
            graph.node_near(site, q, own_r + 1).is_some_and(|j| field.contains_key(&j))
        };
        cut = cut_off_point((t.x, t.z), (dk.x, dk.z), dc, d_safe, tac.cut_fraction, ring, &mut ok);
        if let Some(c) = cut {
            let to_adults = s.moving && {
                let (kx, kz, _) = gap(t.x, t.z, dk.x, dk.z, ring);
                angle_between(t.heading, kx.atan2(kz)) < FRAC_PI_2
            };
            let reasons = f64::from(u8::from(s.young || to_adults) + u8::from(driving));
            let taken = mates.iter().any(|m| gap(m.x, m.z, c.0, c.1, ring).2 <= tac.cut_taken_m);
            sc[ix("cut_off")] = tac.w_cut_off + tac.cut_bonus * reasons - if taken { tac.role_damp } else { 0.0 };
        }
    }
    // Intercept: a moving target it can head off.
    let mut intercept = None;
    let speed = fauna.config.manifest(founder).cruise_m_per_s * crate::gait::sprint_forward(&fauna.config, founder);
    let vel = (mem.vel[0], mem.vel[1]);
    let vs = (vel.0 * vel.0 + vel.1 * vel.1).sqrt();
    if s.moving && vs > 1e-6 {
        let (own_r, _) = node_radii(geometry, h, v);
        let graph = fauna.pack_routes(view, founder);
        let tfrom = graph.node_near(t.face(), (t.x, t.z), own_r);
        let width = view.config.width;
        let depth = view.config.depth;
        let mut clear = |q: (f64, f64)| {
            let Some(tf) = tfrom else { return true };
            let site = Site { x: ((q.0 / v).floor() as i64).rem_euclid(i64::from(width)) as u32, y: t.site.1, z: ((q.1 / v).floor().max(0.0) as u32).min(depth.saturating_sub(1)) };
            graph.node_near(site, q, own_r + 1).is_some_and(|j| graph.walk_line_to(tf, (t.x, t.z), q, j))
        };
        if let Some((q, _)) = intercept_point((a.pose.x, a.pose.z), (t.x, t.z), vel, speed, tac.lead_max_s, tac.lead_min_s, ring, &mut clear) {
            // Worth it when straight pursuit lags: crossing its path or as fast as it.
            let (bx, bz, _) = gap(a.pose.x, a.pose.z, t.x, t.z, ring);
            let crossing = angle_between(vel.0.atan2(vel.1), bx.atan2(bz)) > tac.intercept_cross_deg.to_radians();
            if crossing || vs >= tac.intercept_pace_ratio * speed {
                intercept = Some(q);
                sc[ix("intercept")] = tac.w_intercept;
            }
        }
    }
    // Encircle and harry: a kicker standing at bay.
    let others: Vec<(f64, f64)> = mates.iter().map(|m| (m.x, m.z)).chain(kin.iter().map(|k| (k.x, k.z))).collect();
    let mut opening = false;
    if s.kicker && s.at_bay {
        sc[ix("encircle")] = tac.w_encircle;
        let ringed = 1 + mates.iter().filter(|m| gap(m.x, m.z, t.x, t.z, ring).2 <= da).count() as u32;
        let faces_other = mates.iter().any(|m| faces(&t, m.x, m.z, tac.harry_face_deg, ring)) && !faces(&t, a.pose.x, a.pose.z, tac.harry_face_deg, ring);
        let cooling = mem.kick_seen(t.id).is_some_and(|at| now.saturating_sub(at) as f64 * DT <= tac.kick_recovery_s);
        opening = faces_other || cooling;
        if ringed >= tac.harry_hounds && opening {
            sc[ix("harry")] = tac.w_harry;
        }
    }
    Options { scores: sc, cut, intercept, dc, da, others, opening }
}

/// **The chase** of a committed hound: its options scored, one held (`hold_s`,
/// `switch_margin`), its skill run. `None` when it has no target it senses.
#[allow(clippy::too_many_arguments)]
fn chase(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, own: u64, tac: &Tactics, pk: &Pack, h: &crate::Hunter, geometry: &crate::Body, mem: Hound) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let now = fauna.tick;
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let p = assess_of(fauna, i, mem.target?)?;
    let t = p.track;
    let subject = Subject::of_track(&t);
    let o = options(fauna, i, view, tac, pk, h, geometry, &mem, &p);
    let knobs = fauna.config.founder(founder).intent;
    let valid: Vec<bool> = o.scores.iter().map(|s| s.is_finite()).collect();
    let probs = crate::intent::probabilities(&o.scores, &valid, knobs.temperature);
    let proposal = (0..probs.len()).filter(|&k| valid[k]).fold(None::<usize>, |b, k| match b {
        Some(j) if probs[j] >= probs[k] => Some(j),
        _ => Some(k),
    })?;
    // Its own hold (the intent record rides along: no rule reads it).
    let held_before = usize::from(mem.held);
    let early = mem.held_on != Some(t.id);
    let age = now.saturating_sub(mem.held_since) as f64 * DT;
    let held = crate::intent::arbitrate(&knobs, &valid, &probs, held_before, age, early);
    if held != held_before || early {
        let mm = fauna.hounds.entry(a.id).or_default();
        mm.held = held as u8;
        mm.held_since = now;
        mm.held_on = Some(t.id);
    }
    let name = VEILHOUND.name(held);
    let pname = VEILHOUND.name(proposal);
    let call = fauna.hound_teach.get(&a.id).map_or(0, |x| x.call);
    teach(fauna, a.id, pname, name, subject, call);
    let top = crate::gait::sprint_forward(&fauna.config, founder);
    let act = match name {
        "cut_off" if o.cut.is_some() => {
            let c = o.cut.expect("checked");
            if let Some(act) = close_in(fauna, view, founder, &a, &t, geometry, h, pk, ring) {
                act
            } else if distance(&a, c.0, c.1, ring) <= tac.arrive_m {
                face_point(&a, t.x, t.z, ring, 1.0)
            } else {
                walk_to(fauna, view, founder, &a, geometry, h, pk, c, t.site.1, top, tac.arrive_m, 1.0)
            }
        }
        "intercept" if o.intercept.is_some() => {
            if let Some(act) = close_in(fauna, view, founder, &a, &t, geometry, h, pk, ring) {
                act
            } else {
                walk_to(fauna, view, founder, &a, geometry, h, pk, o.intercept.expect("checked"), t.site.1, top, 0.0, 1.0)
            }
        }
        "harry" | "encircle" => {
            // Harry: dart in for `harry_s` on an opening, then out to the ring; a new dart no
            // sooner than `harry_s` after the last one ended.
            let since = now.saturating_sub(mem.dart_tick) as f64 * DT;
            let mut darting = name == "harry" && mem.dart_tick > 0 && since <= tac.harry_s;
            if name == "harry" && o.opening && (mem.dart_tick == 0 || since > 2.0 * tac.harry_s) {
                fauna.hounds.entry(a.id).or_default().dart_tick = now;
                darting = true;
            }
            if darting && a.strike.is_none() {
                close_in(fauna, view, founder, &a, &t, geometry, h, pk, ring).unwrap_or_else(|| walk_to(fauna, view, founder, &a, geometry, h, pk, (t.x, t.z), t.site.1, top, 0.0, 1.0))
            } else {
                let vel = encircle_velocity((a.pose.x, a.pose.z), (t.x, t.z), &o.others, o.dc, o.da, tac, side_of(a.id), ring);
                let speed = (vel.0 * vel.0 + vel.1 * vel.1).sqrt();
                if speed < tac.still_pace {
                    face_point(&a, t.x, t.z, ring, 0.0)
                } else {
                    let m = fauna.config.manifest(founder);
                    let step = (m.cruise_m_per_s * m.controller_period_s).max(tac.arrive_m);
                    let goal = (a.pose.x + vel.0 / speed * step, a.pose.z + vel.1 / speed * step);
                    walk_to(fauna, view, founder, &a, geometry, h, pk, goal, a.site.y, speed.min(1.0), 0.0, 0.0)
                }
            }
        }
        _ => drive_step(fauna, i, view, own, pk, h, geometry, &t),
    };
    Some(timed(fauna, tac, &a, geometry, h, &t, &memory(fauna, a.id), ring, act))
}

/// **Drive**: the old driver's chase on the target as this hound senses it — the last
/// metres to its stand-off, its route, the stalk's quiet beyond `commit_m` and the rush.
#[allow(clippy::too_many_arguments)]
fn drive_step(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, own: u64, pk: &Pack, h: &crate::Hunter, geometry: &crate::Body, t: &Track) -> Actions {
    let a = fauna.animals[i];
    let founder = a.founder.expect("a pack lineage");
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    if let Some(act) = close_in(fauna, view, founder, &a, t, geometry, h, pk, ring) {
        return act;
    }
    let cruise = fauna.config.manifest(founder).cruise_m_per_s;
    let act = match route_step(fauna, view, founder, own, pk, cruise, &a, t, geometry, h, (t.x, t.z)) {
        Some(p) => route_go(fauna, view, &a, founder, p, ring, 1.0),
        None => go_to(&a, t.x, t.z, ring, 0.5 * geometry.length_m, 1.0),
    };
    let d = distance(&a, t.x, t.z, ring);
    match crate::vigil::stalk(fauna, &a, t, d, act.forward) {
        Some(forward) => {
            if forward > 1.0 {
                // The rush: the hunt cue.
                let call = call_kind(fauna, i, HUNT_CUE);
                if call > 0 {
                    fauna.hound_teach.entry(a.id).or_default().call = call;
                }
            } else if let Some(t2) = fauna.hound_teach.get_mut(&a.id) {
                if t2.held == "drive" && forward < act.forward {
                    t2.held = "stalk";
                    t2.proposal = "stalk";
                    hint(fauna, a.id, "stalk");
                }
            }
            Actions { forward, ..act }
        }
        None => act,
    }
}

/// **Search** (nothing to hunt): a hunt cue it heard (toward the caller), a regroup howl it
/// remembers, the scent, the nearest packmate it sees beyond `follow_m`, the patrol; at its
/// den, hungry, it howls now and then.
#[allow(clippy::too_many_arguments)]
fn search(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, tac: &Tactics, pk: &Pack, h: &crate::Hunter, geometry: &crate::Body, held: Actions, occupancy: Option<crate::senses::Occ<'_>>, den: Site) -> Actions {
    let a = fauna.animals[i];
    let founder = a.founder.expect("a pack lineage");
    let now = fauna.tick;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let mem = memory(fauna, a.id);
    let phys = *fauna.config.founder(founder);
    let lost = phys.intent.subject_lost_s;
    label(fauna, a.id, "search", Subject::None);
    // At its den, hungry: the regroup howl now and then.
    let (dcx, dcz) = centre(den, v);
    if tac.den_howl_s > 0.0 && distance(&a, dcx, dcz, ring) <= pk.den_radius_m && (mem.den_howl_tick == 0 || now.saturating_sub(mem.den_howl_tick) as f64 * DT >= tac.den_howl_s) {
        let call = call_kind(fauna, i, REGROUP_HOWL);
        if call > 0 {
            fauna.hounds.entry(a.id).or_default().den_howl_tick = now;
            teach(fauna, a.id, "search", "search", Subject::None, call);
        }
    }
    if tac.heed_cue {
        if let Some(r) = cue_heard(fauna, a.id, phys.calls.keep_s) {
            if distance(&a, r.x, r.z, ring) > tac.follow_m {
                return walk_to(fauna, view, founder, &a, geometry, h, pk, (r.x, r.z), a.site.y, 1.0, tac.follow_m, 0.0);
            }
        }
    }
    if tac.heed_howl && mem.howl_tick > 0 && now.saturating_sub(mem.howl_tick) as f64 * DT <= tac.howl_keep_s && distance(&a, mem.howl[0], mem.howl[1], ring) > tac.follow_m {
        return walk_to(fauna, view, founder, &a, geometry, h, pk, (mem.howl[0], mem.howl[1]), a.site.y, 1.0, tac.follow_m, 0.0);
    }
    if let Some((x, z)) = scent_step(fauna, i, founder) {
        return go_to(&a, x, z, ring, 0.0, 1.0);
    }
    // Together: the nearest packmate it sees, when farther than the follow radius.
    let mates = mates_of(fauna, a.id, tac, lost);
    if let Some(m) = mates.iter().min_by(|x, y| x.d_m.total_cmp(&y.d_m).then(x.id.cmp(&y.id))) {
        if distance(&a, m.x, m.z, ring) > pk.follow_radius_m {
            return walk_to(fauna, view, founder, &a, geometry, h, pk, (m.x, m.z), m.site.1, 1.0, pk.follow_radius_m, 0.0);
        }
    }
    if fauna.config.hunt.patrol && held.feed > 0.0 && held.forward <= fauna.config.teachers.stalk && !crate::hunt::mouth_on_carcass(fv, view, &a, geometry) {
        if let Some((go, _)) = crate::hunt::steer(fauna, i, view, fv, occupancy) {
            return go;
        }
    }
    held
}

// ------------------------------------------------------------------ strikes and couplings

/// **What a tactics hound may strike** (§4.2, §11.4): its target, or a young body it senses
/// whose one-hound `p_kill` is at least `substitute_p_kill` (an explicit substitution).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct StrikeRule {
    pub target: Option<u64>,
    pub substitutes: Vec<u64>,
}

impl StrikeRule {
    pub(crate) fn allows(&self, id: u64) -> bool {
        self.target == Some(id) || self.substitutes.contains(&id)
    }
}

/// The strike rule of body `i`: `None` (no restriction) unless it is a grown member of a pack
/// whose tactics are on. A heuristic hound's target is its memory's; a brain's, its intent
/// record's body subject (no subject: no restriction, the old capsule rule).
pub(crate) fn strike_rule(fauna: &Fauna, i: usize) -> Option<StrikeRule> {
    let a = &fauna.animals[i];
    let f = a.founder?;
    let phys = fauna.config.founder(f);
    let pk = phys.pack?;
    if !pk.tactics.enabled || a.pack.is_none_or(|m| m.pup) {
        return None;
    }
    let target = if fauna.controllers.layer(a.id) == crate::Layer::Heuristic {
        memory(fauna, a.id).target
    } else {
        Some(a.intent.subject.body()?)
    };
    let tac = pk.tactics;
    let lost = phys.intent.subject_lost_s;
    let substitutes = prey_of(fauna, a.id, lost)
        .iter()
        .filter(|t| looks_young(&fauna.config, &tac, t) && Some(t.id) != target)
        .filter(|t| assess_of(fauna, i, t.id).is_some_and(|p| p.assess.p_one >= tac.substitute_p_kill))
        .map(|t| t.id)
        .collect();
    Some(StrikeRule { target, substitutes })
}

/// **The last metres** (the pack's [`super::close_in`]) for a tactics hound: with
/// [`Tactics::lead_close`] and the target seen moving, its forward gains the target's pace
/// away from it (its own velocity estimate over cruise), up to its sprint, while it walks on
/// the target (a pivot or a step off is left as it is).
#[allow(clippy::too_many_arguments)]
fn close_in(fauna: &mut Fauna, view: &VoxelView<'_>, founder: crate::Founder, a: &Animal, t: &Track, geometry: &crate::Body, h: &crate::Hunter, pk: &Pack, ring: f64) -> Option<Actions> {
    let act = super::close_in(fauna, view, founder, a, t, geometry, h, pk, ring)?;
    if !pk.tactics.lead_close || !t.moving || act.forward <= 0.0 {
        return Some(act);
    }
    let mem = memory(fauna, a.id);
    let (dx, dz, d) = gap(a.pose.x, a.pose.z, t.x, t.z, ring);
    let away = if d > 1e-9 { (mem.vel[0] * dx + mem.vel[1] * dz) / d } else { 0.0 };
    let cruise = fauna.config.manifest(founder).cruise_m_per_s;
    if !(away > 0.0) || !(cruise > 0.0) {
        return Some(act);
    }
    let top = crate::gait::sprint_forward(&fauna.config, founder);
    Some(Actions { forward: (act.forward + away / cruise).min(top.max(act.forward)), ..act })
}

/// The horizontal distance from `p` to the segment `a → b`, on the ring.
fn to_segment(a: (f64, f64), b: (f64, f64), p: (f64, f64), ring: f64) -> f64 {
    let (bx, bz, _) = gap(a.0, a.1, b.0, b.1, ring);
    let (px, pz, _) = gap(a.0, a.1, p.0, p.1, ring);
    let len2 = bx * bx + bz * bz;
    let k = if len2 > 0.0 { ((px * bx + pz * bz) / len2).clamp(0.0, 1.0) } else { 0.0 };
    ((px - k * bx).powi(2) + (pz - k * bz).powi(2)).sqrt()
}

/// **Strike timing** (VH-FIX, [`Tactics::time_strikes`]): for hound `a` (body `geometry`,
/// strike `h`) on target track `t` with its velocity estimate `vel` (its own sightings),
/// `Some(lands)` when the target sits in its strike capsule as it senses it — the jaw's sweep
/// from where it stands, `strike_reach` ahead — and `None` when it does not. `lands`: the
/// sweep of a lunge begun now (the windup standing, then `strike_reach` forward) meets the
/// target carried on at `vel` over the track's age, the windup and the lunge. Reads its own
/// body and what it senses (the track's place, size and age).
pub(crate) fn lunge_lands(config: &crate::FaunaConfig, a: &Animal, geometry: &crate::Body, h: &crate::Hunter, t: &Track, vel: (f64, f64), now: u64, ring: f64) -> Option<bool> {
    let Some(&pf) = crate::Founder::ALL.get(usize::from(t.lineage)) else { return None };
    let pg = config.founder(pf).body_at(body_est(config, t));
    let radii = geometry.footprint_radius() + pg.footprint_radius();
    let (sin, cos) = a.pose.heading_rad.sin_cos();
    let jaw = h.jaw_forward_length_fraction * geometry.length_m;
    let reach = h.strike_reach_length_fraction * geometry.length_m;
    let from = (a.pose.x + jaw * sin, a.pose.z + jaw * cos);
    let to = (from.0 + reach * sin, from.1 + reach * cos);
    if to_segment(from, to, (t.x, t.z), ring) > radii {
        return None;
    }
    let lead = age_s(t, now) + h.windup_s + h.lunge_s;
    let there = (t.x + vel.0 * lead, t.z + vel.1 * lead);
    Some(to_segment(from, to, there, ring) <= radii)
}

/// [`lunge_lands`] applied to a tactics hound's actions: with the strike timed and its target
/// in the capsule where a lunge now would miss, the strike is held (`feed` 0) and it keeps
/// closing.
#[allow(clippy::too_many_arguments)]
fn timed(fauna: &Fauna, tac: &Tactics, a: &Animal, geometry: &crate::Body, h: &crate::Hunter, t: &Track, mem: &Hound, ring: f64, act: Actions) -> Actions {
    if !tac.time_strikes || act.feed <= 0.0 || a.strike.is_some() {
        return act;
    }
    let vel = if t.moving { (mem.vel[0], mem.vel[1]) } else { (0.0, 0.0) };
    match lunge_lands(&fauna.config, a, geometry, h, t, vel, fauna.tick, ring) {
        Some(false) => Actions { feed: 0.0, ..act },
        _ => act,
    }
}

/// **A prey seen running** (§2.1) for the stalk's rush: with tactics on, its track shows it
/// moving away from the hound (`flee_deg`), never its flight state; `None` with tactics off
/// (the old tactic reads the flight state).
pub(crate) fn seen_running(fauna: &Fauna, a: &Animal, t: &Track) -> Option<bool> {
    let tac = a.founder.and_then(|f| fauna.config.founder(f).pack).map(|p| p.tactics).filter(|t| t.enabled)?;
    let ring = fauna.world_geometry.map_or(f64::INFINITY, |(r, _)| r);
    let (dx, dz, d) = gap(a.pose.x, a.pose.z, t.x, t.z, ring);
    Some(t.moving && d > 1e-9 && angle_between(t.heading, dx.atan2(dz)) < tac.flee_deg.to_radians())
}

/// **Who works a hunkered prey** (§11.6, HERD's pry): hound `o` attacks `prey` by its own
/// local attack state — its tactics memory's target — with tactics on; with them off, the
/// old pack target.
pub(crate) fn attacks(fauna: &Fauna, o: &Animal, prey: u64) -> bool {
    let Some(m) = o.pack else { return false };
    let on = o.founder.and_then(|f| fauna.config.founder(f).pack).is_some_and(|p| p.tactics.enabled);
    if on {
        fauna.hounds.get(&o.id).is_some_and(|h| h.target == Some(prey))
    } else {
        fauna.packs.iter().any(|r| r.id == m.pack && r.target == Some(prey))
    }
}

/// **The Social block's target and feast** (§2.1: no true pose): a pack body's target at its
/// last-known point in its own record (its tactics memory's target, or with tactics off the
/// pack target it pursues), and a carcass it knows (tactics on: its memory's; off: the
/// pack's feast).
pub(crate) fn social_target(fauna: &Fauna, a: &Animal) -> Option<(f64, f64)> {
    let m = a.pack?;
    let on = a.founder.and_then(|f| fauna.config.founder(f).pack).is_some_and(|p| p.tactics.enabled);
    let tid = if on {
        memory(fauna, a.id).target
    } else {
        fauna.pack(m.pack).and_then(|p| p.target)
    }?;
    let p = fauna.percept(a.id)?;
    let t = p.track(tid).copied().or_else(|| Some(p.focus).filter(|f| f.known && f.id == tid))?;
    Some((t.x, t.z))
}

pub(crate) fn social_feast(fauna: &Fauna, a: &Animal) -> Option<Site> {
    let m = a.pack?;
    let on = a.founder.and_then(|f| fauna.config.founder(f).pack).is_some_and(|p| p.tactics.enabled);
    if on { memory(fauna, a.id).carcass } else { fauna.pack(m.pack).and_then(|p| p.feast) }
}

/// Tests of the old pack tactic (the comparison arm) pin it: tactics off.
#[cfg(test)]
pub(crate) fn old_tactic(c: &mut crate::FaunaConfig) {
    if let Some(p) = c.founders[crate::Founder::Chorister.index()].pack.as_mut() {
        p.tactics.enabled = false;
    }
}

#[cfg(test)]
#[path = "tactics_tests.rs"]
mod tactics_tests;
