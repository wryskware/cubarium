//! **Intents, subjects and their records** (CONTRACTS, `design/behaviour-ai-2026-10-11.md`
//! §2.2, §10.2, §11.1 and §11.3).
//!
//! Every lineage has a **menu**: the named intents its top layer (and its rules, as
//! teachers) choose between, plus two spare slots kept masked for the later
//! self-invented-intent study. A plain lineage's menu has one item and no spares, so its
//! brain is today's brain (§10.1).
//!
//! On each animal the [`IntentRecord`] keeps, apart from one another:
//! - the **proposal**: the teacher's best intent at this sample, before any hold
//!   (what the top layer imitates, §11.3);
//! - the **held** intent, its **subject** and the tick its hold began (after the shared
//!   [`arbitrate`] rule: `hold_s`, then `switch_margin` on probabilities at a fixed
//!   temperature);
//! - the **executed** actions (the held local actions plus the discrete ones);
//! - the **override** that replaced the steering, if a physical one did (DROP, wade-out,
//!   the shallows, a hit). [`crate::Steer`] stays the executed steering source.
//!
//! The records ride along: no rule reads them, so a world with today's heuristics behaves
//! as it did before they existed.

use serde::{Deserialize, Serialize};

use crate::manifest::Founder;

/// The spare intents a menu lineage carries past its named ones, kept masked.
pub const SPARE_INTENTS: usize = 2;
/// The most intents a menu holds, spares included.
pub const MAX_MENU: usize = 16;

/// What an intent needs for a subject (§11.1's intent–subject validity).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    /// No subject: it is held as none.
    Nothing,
    /// A subject if one fits, else none.
    Optional(Fits),
    /// A subject that fits, or the intent is masked.
    Required(Fits),
}

/// Which candidate rows fit an intent as its subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fits {
    /// A kin body.
    Kin,
    /// A threat body.
    Threat,
    /// A prey body.
    Prey,
    /// Any body.
    Body,
    /// A place of this kind.
    Place(PlaceKind),
}

/// Who may hold an intent: by age, relative to the lineage's adult body (§3.1's "who").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    All,
    /// Under [`IntentKnobs::adult_fraction`] of `body_max`.
    Young,
    /// At or over it.
    Adult,
}

/// One named intent: its name, what it needs for a subject, and who may hold it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    pub name: &'static str,
    pub need: Need,
    pub who: Who,
}

const fn o(name: &'static str, need: Need, who: Who) -> Choice {
    Choice { name, need, who }
}

/// **A lineage's menu** (§2.2): its named intents, then `spare` masked slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Menu {
    pub named: &'static [Choice],
    pub spare: usize,
}

impl Menu {
    /// Intents in all, spares included.
    pub fn len(&self) -> usize {
        self.named.len() + self.spare
    }

    /// Never empty: a plain menu has one item.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// A one-item menu: today's brain, no top layer, no subjects.
    pub fn plain(&self) -> bool {
        self.len() == 1
    }

    /// The index of the named intent `name`.
    pub fn index(&self, name: &str) -> Option<usize> {
        self.named.iter().position(|o| o.name == name)
    }

    /// The name of intent `k` (`spare0`, `spare1` past the named ones).
    pub fn name(&self, k: usize) -> &'static str {
        const SPARES: [&str; SPARE_INTENTS] = ["spare0", "spare1"];
        self.named.get(k).map_or_else(|| SPARES.get(k - self.named.len()).copied().unwrap_or("?"), |o| o.name)
    }

    /// The option of intent `k`, `None` for a spare.
    pub fn option(&self, k: usize) -> Option<&Choice> {
        self.named.get(k)
    }
}

use Fits::{Kin, Place, Prey, Threat};
use Need::{Nothing, Optional, Required};
use Who::{Adult, All, Young};

/// The plain lineages' one item.
pub const PLAIN: Menu = Menu { named: &[o("live", Nothing, All)], spare: 0 };

/// **The seedporter** (§10.2's label sources: its modes and the `Steer` readout).
pub const SEEDPORTER: Menu = Menu {
    named: &[
        o("forage", Nothing, All),
        o("stay_with_troop", Optional(Kin), All),
        o("roost", Optional(Place(PlaceKind::RestSpot)), All),
        o("groom", Optional(Kin), All),
        o("follow_mother", Required(Kin), Young),
        o("flee_to_trees", Optional(Threat), All),
        o("rest", Optional(Place(PlaceKind::RestSpot)), All),
        // YOUNG-ROAM (§3.5): a weaned young's excursion.
        o("explore", Optional(Place(PlaceKind::ExcursionGoal)), Young),
    ],
    spare: SPARE_INTENTS,
};

/// **The loftstrider** (§3.1's menu).
pub const LOFTSTRIDER: Menu = Menu {
    named: &[
        o("graze", Nothing, All),
        o("stay_with_family", Optional(Kin), All),
        o("watch", Required(Threat), All),
        o("go_to_adults", Required(Kin), Young),
        o("go_to_calf", Required(Kin), Adult),
        o("guard", Required(Threat), Adult),
        o("move_off", Optional(Threat), All),
        o("rest", Optional(Place(PlaceKind::RestSpot)), All),
        // YOUNG-ROAM (§3.5): a calf's excursion.
        o("explore", Optional(Place(PlaceKind::ExcursionGoal)), Young),
    ],
    spare: SPARE_INTENTS,
};

/// **The Veilhound** (§4.2's menu).
pub const VEILHOUND: Menu = Menu {
    named: &[
        o("rest", Optional(Place(PlaceKind::RestSpot)), All),
        o("den", Optional(Place(PlaceKind::Den)), All),
        o("search", Nothing, All),
        o("stalk", Required(Prey), Adult),
        o("test", Required(Prey), Adult),
        o("drive", Required(Prey), Adult),
        o("cut_off", Required(Prey), Adult),
        o("intercept", Required(Prey), Adult),
        o("encircle", Required(Prey), Adult),
        o("harry", Required(Prey), Adult),
        o("give_up", Optional(Prey), Adult),
        o("feed", Nothing, All),
    ],
    spare: SPARE_INTENTS,
};

/// **The lanternjaw** (§5, §10.2: `Steer::Patrol`, `Steer::Yield`, the ambush and lure).
pub const LANTERNJAW: Menu = Menu {
    named: &[
        o("ambush", Nothing, All),
        o("patrol", Nothing, All),
        o("feed", Nothing, All),
        o("give_way", Optional(Threat), All),
        o("flee", Optional(Threat), All),
        o("rest", Nothing, All),
    ],
    spare: SPARE_INTENTS,
};

/// A lineage's menu: the four menu lineages of topic 4, and one item for the rest.
pub fn menu_of(founder: Founder) -> &'static Menu {
    match founder {
        Founder::Seedporter => &SEEDPORTER,
        Founder::Loftstrider => &LOFTSTRIDER,
        Founder::Chorister => &VEILHOUND,
        Founder::Lanternjaw => &LANTERNJAW,
        Founder::Blind | Founder::Browser | Founder::Capgnawer | Founder::RippleSnail | Founder::Bellwing => &PLAIN,
    }
}

/// **A place's kind** (§11.1): a rest spot, an excursion goal, a guard or cut-off point, and
/// the home places the rules already walk to (a den).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceKind {
    #[default]
    RestSpot,
    ExcursionGoal,
    GuardPoint,
    CutOffPoint,
    Den,
}

impl PlaceKind {
    pub const ALL: [PlaceKind; 5] =
        [PlaceKind::RestSpot, PlaceKind::ExcursionGoal, PlaceKind::GuardPoint, PlaceKind::CutOffPoint, PlaceKind::Den];

    pub const fn index(self) -> usize {
        match self {
            PlaceKind::RestSpot => 0,
            PlaceKind::ExcursionGoal => 1,
            PlaceKind::GuardPoint => 2,
            PlaceKind::CutOffPoint => 3,
            PlaceKind::Den => 4,
        }
    }
}

/// **A subject** (§11.1): none, a body held by its id, or a place. A body is re-marked to
/// its current candidate row at each sample, so slot order never changes it; its raw id
/// never enters a network. `seen_tick` is when it was last sensed (a place: last confirmed).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Subject {
    #[default]
    None,
    /// A body by id, with when and where (`[x, z, y_m]`; `None`: never by this body) it was
    /// last sensed.
    Body { id: u64, seen_tick: u64, at: Option<[f64; 3]> },
    Place { kind: PlaceKind, x: f64, z: f64, y_m: f64, seen_tick: u64 },
}

impl Subject {
    pub fn body(self) -> Option<u64> {
        match self {
            Subject::Body { id, .. } => Some(id),
            _ => None,
        }
    }

    pub fn is_none(self) -> bool {
        matches!(self, Subject::None)
    }

    /// The body of a track, where and when it was sensed.
    pub fn of_track(t: &crate::Track) -> Subject {
        Subject::Body { id: t.id, seen_tick: t.tick, at: Some([t.x, t.z, t.y_m]) }
    }
}

/// **Why a hold ended early** (§11.3): the top chooses again at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EarlyEnd {
    /// Its body subject died.
    SubjectDead,
    /// Its subject went unsensed for `subject_lost_s`.
    SubjectLost,
    /// Its intent became masked.
    Masked,
    /// A physical emergency ([`Override`]).
    Emergency,
}

/// **A physical override** of the steering (§11.3): the body, not the choice, set the held
/// actions this sample.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Override {
    #[default]
    None,
    /// DROP: a trapped body's jump down.
    Drop,
    /// WADE-OUT: the way out of water past its depth.
    WadeOut,
    /// The shallows reflex: water deeper than it wades.
    Shallows,
    /// Wounded since its last sample.
    Hit,
}

impl Override {
    /// The override a steering source is, if it is a physical one.
    pub fn of_steer(src: crate::Steer) -> Override {
        match src {
            crate::Steer::Drop => Override::Drop,
            crate::Steer::Wade => Override::WadeOut,
            crate::Steer::Shallows => Override::Shallows,
            _ => Override::None,
        }
    }
}

/// **A rest request** through the action interface (§11.4): the body checks eligibility, and
/// its rhythm stays the backstop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestAsk {
    /// No request: the rhythm decides, as it always has.
    #[default]
    Default,
    /// Rest now, if the body may.
    Rest,
    /// Stay up although the rhythm says rest (the heuristics' walk home to the den, a
    /// glider's better roost).
    Stay,
}

/// **The executed actions** of a sample: the held local actions and the discrete ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Executed {
    pub actions: crate::Actions,
    /// A kick started this sample ([`crate::kick`]).
    pub kick: bool,
    /// The call kind emitted this sample, 0 for none, `k + 1` for the lineage's kind `k`.
    pub call: u8,
    pub rest: RestAsk,
}

/// **An animal's intent record** (module note). Saved: a load continues a hold rather than
/// cold-starting it (§11.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IntentRecord {
    /// The teacher's (or brain's) proposal this sample, before the hold.
    pub proposal: u8,
    /// The subject that came with the proposal.
    pub proposal_subject: Subject,
    /// The held intent, after the hold and the margin.
    pub held: u8,
    pub subject: Subject,
    /// The tick the held intent began.
    pub since_tick: u64,
    /// The tick of the last decision sample (0: none yet).
    pub sample_tick: u64,
    pub executed: Executed,
    pub overridden: Override,
    /// Why the hold before this sample ended early, if it did.
    pub ended: Option<EarlyEnd>,
    /// The wound at the last sample: a rise is a hit.
    pub last_wound: f64,
}

impl IntentRecord {
    /// Seconds the held intent has been held at `tick`.
    pub fn hold_age_s(&self, tick: u64) -> f64 {
        tick.saturating_sub(self.since_tick) as f64 * cubarium_voxel::DT
    }
}

/// **The arbitration knobs** of a lineage's top layer and its records (§10.1, §11.1, §11.3).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntentKnobs {
    /// The least time an intent is held before another may replace it, seconds (§2.2's
    /// ~1 s). An early end (subject lost or dead, intent masked, an emergency) skips it.
    pub hold_s: f64,
    /// How much more probable a new intent must be than the held one to replace it, after
    /// the hold (a probability difference, 0..1).
    pub switch_margin: f64,
    /// The fixed softmax temperature the margin's probabilities are taken at, so a change in
    /// logit scale cannot change switching on its own.
    pub temperature: f64,
    /// A subject unsensed for this long becomes none and ends its hold, seconds.
    pub subject_lost_s: f64,
    /// The bounded slots of §11.1: kin (a sensed mother or calf keeps a reserved one), threat
    /// and prey bodies, and place rows.
    pub kin_slots: u32,
    pub threat_slots: u32,
    pub prey_slots: u32,
    pub place_slots: u32,
    /// A body under this fraction of its lineage's `body_max` is young for the menu's "who".
    pub adult_fraction: f64,
}

impl Default for IntentKnobs {
    /// A plain lineage's: no slots.
    fn default() -> IntentKnobs {
        IntentKnobs {
            hold_s: 1.0,
            switch_margin: 0.1,
            temperature: 1.0,
            subject_lost_s: 10.0,
            kin_slots: 0,
            threat_slots: 0,
            prey_slots: 0,
            place_slots: 0,
            adult_fraction: 0.8,
        }
    }
}

/// Each lineage's arbitration knobs: §2.2's ~1 s hold, a 0.1 margin at temperature 1, and
/// §11.1's slots for the menu lineages (four kin, matching §2.1's `kin_tracks` 1 → 4; a
/// hunter's prey slots; one place row). Placeholders, untuned.
pub fn intent_of(founder: Founder) -> IntentKnobs {
    let d = IntentKnobs::default();
    match founder {
        Founder::Seedporter => IntentKnobs { kin_slots: 4, threat_slots: 2, prey_slots: 1, place_slots: 1, ..d },
        Founder::Loftstrider => IntentKnobs { kin_slots: 4, threat_slots: 3, prey_slots: 0, place_slots: 1, ..d },
        Founder::Chorister => IntentKnobs { kin_slots: 4, threat_slots: 1, prey_slots: 3, place_slots: 1, ..d },
        Founder::Lanternjaw => IntentKnobs { kin_slots: 0, threat_slots: 2, prey_slots: 2, place_slots: 0, ..d },
        _ => d,
    }
}

/// Probabilities of `logits` at `temperature` over the `valid` intents (masked ones 0).
pub fn probabilities(logits: &[f64], valid: &[bool], temperature: f64) -> Vec<f64> {
    let t = if temperature.is_finite() && temperature > 0.0 { temperature } else { 1.0 };
    let top = logits
        .iter()
        .zip(valid)
        .filter(|(l, v)| **v && l.is_finite())
        .map(|(l, _)| *l)
        .fold(f64::NEG_INFINITY, f64::max);
    if !top.is_finite() {
        return vec![0.0; logits.len()];
    }
    let e: Vec<f64> = logits
        .iter()
        .zip(valid)
        .map(|(l, v)| if *v && l.is_finite() { ((l - top) / t).exp() } else { 0.0 })
        .collect();
    let sum: f64 = e.iter().sum();
    e.iter().map(|x| x / sum).collect()
}

/// **The shared hold** (§11.3) over proposal probabilities: the held intent stays while it
/// is valid, no early end is flagged and it has been held under `hold_s`; after that, the
/// most probable valid intent replaces it only when more probable by `switch_margin`.
/// Masked intents are never held; with none valid, intent 0 is.
pub fn arbitrate(knobs: &IntentKnobs, valid: &[bool], probs: &[f64], held: usize, hold_age_s: f64, early: bool) -> usize {
    let best = (0..probs.len().min(valid.len()))
        .filter(|&k| valid[k])
        .fold(None::<usize>, |b, k| match b {
            Some(j) if probs[j] >= probs[k] => Some(j),
            _ => Some(k),
        });
    let Some(best) = best else { return 0 };
    let held_ok = held < valid.len() && valid[held];
    if !held_ok || early {
        return best;
    }
    if hold_age_s < knobs.hold_s {
        return held;
    }
    if probs[best] - probs.get(held).copied().unwrap_or(0.0) > knobs.switch_margin {
        best
    } else {
        held
    }
}

/// The intent mask of a menu for a body: spares masked, the "who" applied, and an intent
/// that requires a subject valid only while some candidate fits it (`fits(k)`).
pub fn intent_mask(menu: &Menu, young: bool, fits: impl Fn(Fits) -> bool) -> Vec<bool> {
    (0..menu.len())
        .map(|k| match menu.option(k) {
            None => false,
            Some(o) => {
                let who = match o.who {
                    Who::All => true,
                    Who::Young => young,
                    Who::Adult => !young,
                };
                who && match o.need {
                    Need::Required(f) => fits(f),
                    _ => true,
                }
            }
        })
        .collect()
}

/// **The teacher's label** for a sample: an intent name of its lineage's menu (none where
/// the rules' mode has no clear name, and then the last proposal stands) and the subject it
/// is about.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Teach {
    pub name: Option<&'static str>,
    pub subject: Subject,
    /// A teacher that runs its own hold (VH-TACTICS, [`crate::HoundTeach`]): the intent it
    /// held, recorded as held without a second hold over the proposal.
    pub held: Option<&'static str>,
    /// The call it made this sample (0 none, `k + 1` its lineage's kind `k`).
    pub call: u8,
    /// LS-DEFEND: it started an aimed kick this sample ([`crate::kick`]).
    pub kick: bool,
}

/// The nearest track of `class` in body `id`'s record, as a body subject.
fn nearest(fauna: &crate::Fauna, id: u64, class: u8) -> Subject {
    fauna
        .percepts
        .get(&id)
        .map(|p| p.best[usize::from(class)])
        .filter(|t| t.known)
        .map_or(Subject::None, |t| Subject::of_track(&t))
}

/// Body `id` as body `me`'s subject: where its record last sensed it, else unplaced.
fn body_subject(fauna: &crate::Fauna, me: u64, id: u64) -> Subject {
    fauna.percepts.get(&me).and_then(|p| p.track(id)).map_or(
        Subject::Body { id, seen_tick: fauna.tick, at: None },
        Subject::of_track,
    )
}

/// **Read the rules' mode as an intent** (§10.2's label sources) for heuristic body `i`
/// after the write side settled its steering `src` and `actions`. `herded`: the herd pull
/// bent its walk; `at_carcass`: its controller is eating carrion. Only the clear mappings:
/// - seedporter: flight → `flee_to_trees`, roost, groom, its mother → `follow_mother`, the
///   herd pull → `stay_with_troop`, its own steering and the walks to food → `forage`, a
///   rest bout → `rest`;
/// - loftstrider: its defence layer's own labels ([`crate::defend::steer`]); else its ground
///   response answering a heeded alarm → `stay_with_family`, flight
///   standing (no forward) → `watch`, flight walking its motor refused → `rest`, flight
///   walking → `move_off`, the herd pull →
///   `stay_with_family`, its own steering standing still in a rest bout or sated → `rest`,
///   else its own steering → `graze`;
/// - Veilhound: the pack tactic's own reading ([`crate::Fauna::pack_hint`]: rest, den,
///   feed, stalk, drive — the flankers too —, else search); its own steering → `search`;
/// - lanternjaw (BRAIN-FIX step 2), from the patrol's own state ([`crate::hunt::steer`],
///   whose answer sets [`crate::HuntMemory::mode`] and reads the eating latch): fed, its
///   eating latch off, it holds still to digest → `rest`; going to or eating at a carcass
///   ([`crate::HuntMode::Meal`]) → `feed`; waiting still on a patch with the lure lit, or
///   turned and stalking to a prey it engages from there ([`crate::HuntMode::Wait`]) →
///   `ambush`; choosing or walking to a patch → `patrol`. The carcass yield → `give_way`.
///   Under its own (ambush) heuristic, the patrol off: at a carcass → `feed`, a rest bout →
///   `rest`, else `ambush`. It has no other threat response — the prey flee rule excludes
///   it and the yield starts only at a carcass — so nothing maps to `flee`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn teacher_label(
    fauna: &crate::Fauna,
    i: usize,
    founder: Founder,
    src: crate::Steer,
    actions: crate::Actions,
    herded: bool,
    at_carcass: bool,
) -> Teach {
    use crate::Steer as S;
    use crate::perception::class;
    let a = &fauna.animals[i];
    let resting = a.rest_ticks > 0 || a.state == crate::State::Resting && actions == crate::Actions::REST;
    let still = actions.forward <= fauna.config.teachers.forage_stay_forward;
    let t = |name: &'static str, subject: Subject| Teach { name: Some(name), subject, ..Teach::default() };
    match founder {
        Founder::Seedporter => match src {
            S::Flee => t("flee_to_trees", nearest(fauna, a.id, class::THREAT)),
            S::Roost => t("roost", Subject::None),
            S::Groom => t("groom", Subject::None),
            S::Nurse => t("follow_mother", a.mother.map_or(Subject::None, |m| body_subject(fauna, a.id, m))),
            S::Explore => t("explore", fauna.excursions.get(&a.id).map_or(Subject::None, crate::roam::subject)),
            S::Heuristic | S::Search | S::Leave | S::Fruit | S::Light if herded => {
                t("stay_with_troop", nearest(fauna, a.id, class::KIN))
            }
            // BRAIN-REST: its `rest` is the pause, a rest bout it holds still in. A glider sleeps
            // only perched (`roost`), so on open ground its heuristic walks on through a rest
            // bout; that walk is labelled what it is, `forage` (traced: 382 of 465 rest-labelled
            // seconds walked).
            S::Heuristic if resting && still => t("rest", Subject::None),
            S::Heuristic | S::Search | S::Leave | S::Fruit | S::Light => t("forage", Subject::None),
            _ => Teach::default(),
        },
        Founder::Loftstrider => match src {
            // LS-DEFEND: the defence layer's own proposal, hold and call; under the hold of a
            // calm intent, its proposal only.
            _ if fauna.ls_teach.contains_key(&a.id) => fauna.ls_teach[&a.id],
            S::Explore => t("explore", fauna.excursions.get(&a.id).map_or(Subject::None, crate::roam::subject)),
            // YOUNG-ROAM: a mother walking to her own calf's bleat.
            S::Answer => t("go_to_calf", Subject::None),
            // LS-TEACH: its ground response answering a heeded alarm (CALLS-REST,
            // `calls::reaction`: to the nearest kin it knows, then head up facing the call
            // beside it) is the family gathering, `stay_with_family`. It was read as flight:
            // `watch` standing, `move_off` walking; but those face or leave a threat, and a body
            // that only heard the snort has none in its record, so `watch` (which needs one)
            // was masked (traced on the held-out training world: all 2046 masked `watch`
            // labels, and 401 of 603 `move_off`, were this answer with no threat track).
            S::Flee if fauna.call_answer.get(&a.id) == Some(&0) => t("stay_with_family", nearest(fauna, a.id, class::KIN)),
            S::Flee if still => t("watch", nearest(fauna, a.id, class::THREAT)),
            // LS-MOVEOFF: a flight walk its motor refused (EDGE's pressed clock, [`Fauna::pressed`]:
            // the interval just ended with forward held and `Self.motor_delivery` under
            // `teachers.motor_refused`) is a body standing, awake and taking no food: `rest`, as
            // CLONE-AGREE reads a standing body. It was `move_off`; but its walk off a calm hunter
            // (FEAR's `calm_walk`) or to a distress call, blocked, re-asks every
            // `press_replan_s` and the body stands for minutes (traced on the held-out training
            // world, seed 9: 1528 of 1874 `move_off` labels pressed and still, 1229 of them
            // still past `still_after_s`; the clone read 1852 of 2364 as `rest`). Not `watch`:
            // that faces a threat it does not take as calm, and these face away from a calm one
            // or have none in their record.
            S::Flee if fauna.pressed.get(&a.id).is_some_and(|&n| n > 0) => t("rest", Subject::None),
            S::Flee => t("move_off", nearest(fauna, a.id, class::THREAT)),
            S::Heuristic | S::Search | S::Leave if herded => t("stay_with_family", nearest(fauna, a.id, class::KIN)),
            // CLONE-AGREE: its `rest` is a body standing that takes no food — still in a rest
            // bout, or still with its eating latch off (sated: the latch off, it takes no
            // bite, `step::gut_bite`). Its forager stands a sated body with its mouth asking
            // (`feed` on), so that stand was labelled `graze` (traced on the held-out training
            // world: 12603 of 13046 graze-labelled samples were still, latched off, taking
            // nothing). Walking, or still and hungry (a forage pause), it grazes; walking on
            // through a rest bout too, as the seedporter's.
            S::Heuristic if still && (resting || !a.eating) => t("rest", Subject::None),
            S::Heuristic | S::Search | S::Leave => t("graze", Subject::None),
            _ => Teach::default(),
        },
        Founder::Chorister => match src {
            // VH-TACTICS: the tactic's own proposal, hold and call.
            S::Pack if fauna.hound_teach.contains_key(&a.id) => {
                let h = fauna.hound_teach[&a.id];
                Teach { name: Some(h.proposal), subject: h.subject, held: Some(h.held), call: h.call, kick: false }
            }
            S::Pack => {
                let name = fauna.pack_hint.get(&a.id).copied().unwrap_or("search");
                let target = a.pack.and_then(|m| fauna.pack(m.pack)).and_then(|p| p.target);
                let subject = match (name, target) {
                    ("stalk" | "drive", Some(id)) => body_subject(fauna, a.id, id),
                    ("den", _) => a.pack.and_then(|m| fauna.pack(m.pack)).map_or(Subject::None, |p| {
                        let v = fauna.world_geometry.map_or(1.0, |(_, v)| v);
                        Subject::Place {
                            kind: PlaceKind::Den,
                            x: (f64::from(p.den.x) + 0.5) * v,
                            z: (f64::from(p.den.z) + 0.5) * v,
                            y_m: crate::surface_m(p.den.y, v),
                            seen_tick: fauna.tick,
                        }
                    }),
                    _ => Subject::None,
                };
                Teach { name: Some(name), subject, ..Teach::default() }
            }
            S::Heuristic | S::Search => t("search", Subject::None),
            _ => Teach::default(),
        },
        Founder::Lanternjaw => match src {
            S::Patrol if !a.eating => t("rest", Subject::None),
            S::Patrol if a.hunt.mode == crate::HuntMode::Meal => t("feed", Subject::None),
            S::Patrol if a.hunt.mode == crate::HuntMode::Wait => t("ambush", Subject::None),
            S::Patrol => t("patrol", Subject::None),
            S::Yield => t("give_way", nearest(fauna, a.id, class::THREAT)),
            // BRAIN-FIX-2 step 2: the ambush answers "eat" to carrion at its mouth from taste
            // alone, and the patrol then stands back; with the eating latch off the body takes
            // nothing and digests where it stands (traced: sated, still, stomach falling).
            S::Heuristic if at_carcass && !a.eating => t("rest", Subject::None),
            S::Heuristic if at_carcass => t("feed", Subject::None),
            S::Heuristic if resting => t("rest", Subject::None),
            S::Heuristic => t("ambush", Subject::None),
            _ => Teach::default(),
        },
        _ => t("live", Subject::None),
    }
}

/// **Whether a body subject still holds** at `now` for body `me`: `Err` with the early end
/// when it is dead (no living body has its id) or unsensed past `lost_s`; else the subject
/// with its `seen_tick` refreshed from the record. Places and none always hold.
pub(crate) fn check_subject(fauna: &crate::Fauna, me: u64, subject: Subject, lost_s: f64) -> Result<Subject, EarlyEnd> {
    let Subject::Body { id, seen_tick, .. } = subject else { return Ok(subject) };
    if fauna.animals.binary_search_by_key(&id, |a| a.id).is_err() {
        return Err(EarlyEnd::SubjectDead);
    }
    let fresh = fauna.percepts.get(&me).and_then(|p| p.track(id)).filter(|t| t.tick >= seen_tick).map(Subject::of_track);
    let s = fresh.unwrap_or(subject);
    let Subject::Body { seen_tick: seen, .. } = s else { return Ok(s) };
    if fauna.tick.saturating_sub(seen) as f64 * cubarium_voxel::DT > lost_s {
        return Err(EarlyEnd::SubjectLost);
    }
    Ok(s)
}

/// **Write body `i`'s intent record** for this sample (module note): the proposal (`teach`
/// or, without a name, the last proposal), the shared hold over its one-hot probabilities,
/// the subject's expiry, the executed actions and any physical override. Read by no rule.
pub(crate) fn record(
    fauna: &mut crate::Fauna,
    i: usize,
    founder: Founder,
    src: crate::Steer,
    teach: Teach,
    valid: &[bool],
    executed: Executed,
) {
    let menu = menu_of(founder);
    let knobs = fauna.config.founder(founder).intent;
    let now = fauna.tick;
    let a = &fauna.animals[i];
    let mut rec = a.intent;
    let hit = a.wound > rec.last_wound + 1e-12;
    rec.overridden = match Override::of_steer(src) {
        Override::None if hit => Override::Hit,
        o => o,
    };
    let me = a.id;
    // The proposal before the hold.
    if let Some(k) = teach.name.and_then(|n| menu.index(n)) {
        rec.proposal = k as u8;
        rec.proposal_subject = teach.subject;
    }
    // Early ends: the subject dead or lost, the intent masked, a physical emergency.
    let mut ended = None;
    match check_subject(fauna, me, rec.subject, knobs.subject_lost_s) {
        Ok(s) => rec.subject = s,
        Err(why) => {
            rec.subject = Subject::None;
            ended = Some(why);
        }
    }
    let held = usize::from(rec.held);
    if ended.is_none() && !valid.get(held).copied().unwrap_or(false) {
        ended = Some(EarlyEnd::Masked);
    }
    if ended.is_none() && rec.overridden != Override::None {
        ended = Some(EarlyEnd::Emergency);
    }
    let first = rec.sample_tick == 0 && rec.since_tick == 0;
    if let Some(k) = teach.held.and_then(|n| menu.index(n)) {
        // A teacher that held its own intent (VH-TACTICS): recorded as it held it.
        if k != held || first {
            rec.since_tick = now;
        }
        rec.held = k as u8;
        rec.subject = teach.subject;
    } else {
        let mut probs = vec![0.0; menu.len()];
        if let Some(p) = probs.get_mut(usize::from(rec.proposal)) {
            *p = 1.0;
        }
        let next = arbitrate(&knobs, valid, &probs, held, rec.hold_age_s(now), ended.is_some() || first);
        if next != held || first {
            rec.held = next as u8;
            rec.since_tick = now;
            rec.subject = if next == usize::from(rec.proposal) { rec.proposal_subject } else { Subject::None };
        } else if next == usize::from(rec.proposal) {
            rec.subject = rec.proposal_subject;
        }
    }
    rec.ended = ended;
    let mut executed = executed;
    if teach.call > 0 {
        executed.call = teach.call;
    } else if executed.call == 0 {
        // YOUNG-ROAM: a call its rules made this sample (a young's bleat at an excursion's
        // end) is an executed action too.
        let id = fauna.animals[i].id;
        let n = crate::calls::kinds_of(founder).len();
        if let Some(k) = (0..n).find(|&k| fauna.last_call.get(&(id, k as u8)) == Some(&now)) {
            executed.call = k as u8 + 1;
        }
    }
    executed.kick |= teach.kick;
    rec.executed = executed;
    rec.sample_tick = now;
    rec.last_wound = fauna.animals[i].wound;
    fauna.animals[i].intent = rec;
}

/// Whether body `a` is young for its lineage's menu ([`IntentKnobs::adult_fraction`]).
pub(crate) fn young(fauna: &crate::Fauna, a: &crate::Animal, founder: Founder) -> bool {
    let phys = fauna.config.founder(founder);
    a.body < phys.intent.adult_fraction * phys.core.body_max
}

/// Body `i`'s intent mask for its record: spares masked, the menu's "who" applied, and an
/// intent that requires a subject valid only while one of its candidates fits.
pub(crate) fn body_mask(fauna: &crate::Fauna, i: usize, founder: Founder) -> Vec<bool> {
    let a = &fauna.animals[i];
    let menu = menu_of(founder);
    if menu.plain() {
        return vec![true];
    }
    let c = crate::subjects::candidates(fauna, i, founder, a.intent.subject);
    intent_mask(menu, young(fauna, a, founder), |f| c.any(f))
}

/// **Write a brain's decision** into body `i`'s record: its proposal (the most probable
/// intent), its held intent and the subject its row stands for (`prep`'s candidates), the
/// executed actions and any physical override. The brain ran its own hold.
pub(crate) fn record_brain(
    fauna: &mut crate::Fauna,
    i: usize,
    src: crate::Steer,
    decision: &crate::Decision,
    prep: &crate::subjects::DecisionPrep,
    executed: Executed,
) {
    let now = fauna.tick;
    let a = &fauna.animals[i];
    let mut rec = a.intent;
    let hit = a.wound > rec.last_wound + 1e-12;
    rec.overridden = match Override::of_steer(src) {
        Override::None if hit => Override::Hit,
        o => o,
    };
    let n = prep.menu.min(MAX_MENU);
    let proposal = (0..n).fold(0, |b, k| if decision.proposal[k] > decision.proposal[b] { k } else { b });
    rec.proposal = proposal as u8;
    let subject = decision.subject.and_then(|r| prep.candidates.rows.get(r)).map_or(Subject::None, |c| c.subject);
    rec.proposal_subject = subject;
    let held = decision.intent.min(n.saturating_sub(1));
    if usize::from(rec.held) != held || rec.sample_tick == 0 {
        rec.since_tick = now;
    }
    rec.held = held as u8;
    rec.subject = subject;
    rec.ended = prep.early_end;
    rec.executed = executed;
    rec.sample_tick = now;
    rec.last_wound = fauna.animals[i].wound;
    fauna.animals[i].intent = rec;
}
