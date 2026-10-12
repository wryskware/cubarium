//! **Slots and subject candidates** (CONTRACTS, behaviour-AI §11.1): the bounded kin, threat
//! and prey rows and the place rows a menu lineage's brain points its subject at.
//!
//! Each decision sample a menu body's candidates are read from its own percept record (never
//! a true position): its kin (a recognised mother or calf keeps the first kin slot while
//! sensed), its threats, its prey, nearest first, then its places. A held body subject still
//! within `subject_lost_s` but missing from the latest sensing stays as a **remembered** row in
//! its class, so the subject is re-marked to its current row whatever the slot order; raw ids
//! never enter a row. A row carries what the teacher reads ([`ROW_CHANNELS`]).

use crate::intent::{Fits, Menu, Need, PlaceKind, Subject};
use crate::manifest::Founder;
use crate::observe_v3::{Frame, level, proximity};
use crate::perception::{self, Track, class, posture};
use crate::{Animal, Fauna};

/// One candidate row's channels, in order. Bearing and proximity are the v3 slot's
/// ([`crate::V3_ENCODING_RULE`]); `elevation` is the height difference over
/// [`crate::HEIGHT_REFERENCE_M`], clamped; `heading_*` its heading relative to the observer's;
/// `size` its length as `l / (l + own length)`; `fresh` `1 − age / subject_lost_s`; stance
/// (guard, rush) as sight shows it ([`perception::stance`]: LS-DEFEND sets guard; no lineage
/// shows a rush yet).
pub const ROW_CHANNELS: [&str; 32] = [
    "valid",
    "body",
    "place",
    "sin",
    "cos",
    "proximity",
    "elevation",
    "heading_sin",
    "heading_cos",
    "moving",
    "resting",
    "asleep",
    "guard",
    "rush",
    "lame",
    "size",
    "fresh",
    "seen",
    "heard",
    "felt",
    "mother",
    "calf",
    "kin",
    "other",
    "threat",
    "prey",
    "rest_spot",
    "excursion_goal",
    "guard_point",
    "cut_off_point",
    "den",
    "remembered",
];

/// A row's width `F`.
pub const ROW_WIDTH: usize = ROW_CHANNELS.len();

/// The candidate rows' order and encoding, as the canonical text records it.
pub const CANDIDATE_RULE: &str = "rows=kin(mother|calf first),threat,prey nearest first,places;held_body_kept_remembered;fresh=1-age/subject_lost_s;elev=dy/height_ref;size=l/(l+own_l);empty=0";

/// What a row holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RowKind {
    #[default]
    Empty,
    Body,
    Place,
}

/// One candidate: its kind, the subject it stands for, its row, and what it fits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub kind: RowKind,
    pub subject: Subject,
    pub row: [f64; ROW_WIDTH],
    /// Fits [`Fits::Kin`], [`Fits::Threat`], [`Fits::Prey`] (bodies), or its place kind.
    pub kin: bool,
    pub threat: bool,
    pub prey: bool,
    pub place: Option<PlaceKind>,
}

impl Default for Candidate {
    fn default() -> Candidate {
        Candidate { kind: RowKind::Empty, subject: Subject::None, row: [0.0; ROW_WIDTH], kin: false, threat: false, prey: false, place: None }
    }
}

/// The key of an empty row (never a body's or a place's).
pub const EMPTY_KEY: u64 = u64::MAX;
/// The bit a place key carries (body ids never reach it; founding mother ids use bit 63).
pub const PLACE_KEY_BIT: u64 = 1 << 62;

/// **A subject's stable key** (the TWO-LAYER brain holds its subject by it, re-finding its
/// row after rows reorder; it never enters a network): a body's id; for a place,
/// [`PLACE_KEY_BIT`] with its kind and its point quantised to 0.25 m cells; [`EMPTY_KEY`]
/// for none.
pub fn subject_key(s: Subject) -> u64 {
    match s {
        Subject::None => EMPTY_KEY,
        Subject::Body { id, .. } => id,
        Subject::Place { kind, x, z, .. } => {
            let q = |v: f64| ((v / 0.25).floor() as i64 as u64) & 0x0FFF_FFFF;
            PLACE_KEY_BIT | ((kind.index() as u64) << 56) | (q(x) << 28) | q(z)
        }
    }
}

impl Candidate {
    /// This row's stable key ([`subject_key`]).
    pub fn key(&self) -> u64 {
        subject_key(self.subject)
    }

    /// Whether this row may be a subject that `fits` asks for.
    pub fn fits(&self, f: Fits) -> bool {
        match f {
            Fits::Kin => self.kin,
            Fits::Threat => self.threat,
            Fits::Prey => self.prey,
            Fits::Body => self.kind == RowKind::Body,
            Fits::Place(k) => self.place == Some(k),
        }
    }
}

/// **A body's candidates** at one sample: `K` rows (kin, threat, prey, place slots), empty
/// rows all zero.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Candidates {
    pub rows: Vec<Candidate>,
}

impl Candidates {
    /// Whether any row fits `f`.
    pub fn any(&self, f: Fits) -> bool {
        self.rows.iter().any(|c| c.fits(f))
    }

    /// The row holding `subject` now: a body by its id, a place by its kind and point.
    pub fn row_of(&self, subject: Subject) -> Option<usize> {
        match subject {
            Subject::None => None,
            Subject::Body { id, .. } => self.rows.iter().position(|c| c.subject.body() == Some(id)),
            Subject::Place { .. } => {
                let key = subject_key(subject);
                self.rows.iter().position(|c| c.kind == RowKind::Place && c.key() == key)
            }
        }
    }

    /// Which rows may be intent `k`'s subject under `menu`.
    pub fn valid_for(&self, menu: &Menu, k: usize) -> Vec<bool> {
        let need = menu.option(k).map_or(Need::Nothing, |o| o.need);
        self.rows
            .iter()
            .map(|c| match need {
                Need::Nothing => false,
                Need::Optional(f) | Need::Required(f) => c.fits(f),
            })
            .collect()
    }

    /// The per-intent row masks, row-major: `mask[r * menu.len() + k]` is row `r` valid for
    /// intent `k`.
    pub fn mask(&self, menu: &Menu) -> Vec<bool> {
        let per: Vec<Vec<bool>> = (0..menu.len()).map(|k| self.valid_for(menu, k)).collect();
        let mut out = vec![false; self.rows.len() * menu.len()];
        for (k, rows) in per.iter().enumerate() {
            for (r, &v) in rows.iter().enumerate() {
                out[r * menu.len() + k] = v;
            }
        }
        out
    }

    /// The rows flat, `K × F`.
    pub fn flat(&self) -> Vec<f64> {
        self.rows.iter().flat_map(|c| c.row).collect()
    }
}

/// Whether `a`'s recorded mother is a living body's id (not a founding mother id,
/// [`crate::founding_mother`]).
fn mother_of(a: &Animal) -> Option<u64> {
    a.mother.filter(|&m| !crate::is_founding_mother(m))
}

/// The relation of body `o` (id) to `a`: 0 mother, 1 calf (its recorded mother is `a`), 2 kin
/// of its lineage, 3 other. Recognition of one's own mother and young (§3.5).
fn relation(fauna: &Fauna, a: &Animal, t: &Track) -> usize {
    if mother_of(a) == Some(t.id) {
        return 0;
    }
    let calf = fauna.animals.binary_search_by_key(&t.id, |o| o.id).ok().is_some_and(|k| mother_of(&fauna.animals[k]) == Some(a.id));
    if calf {
        1
    } else if t.class == class::KIN {
        2
    } else {
        3
    }
}

/// A body row from its track (`remembered`: kept for the held subject, not in the latest
/// sensing).
fn body_row(fauna: &Fauna, a: &Animal, frame: &Frame, t: &Track, range_m: f64, lost_s: f64, own_length: f64, remembered: bool) -> Candidate {
    let mut r = [0.0; ROW_WIDTH];
    let age_s = t.age(frame.tick) as f64 * cubarium_voxel::DT;
    let (s, c, d) = frame.toward(t.x, t.z);
    let rel_heading = t.heading - a.pose.heading_rad;
    let rel = relation(fauna, a, t);
    let cl = |v: f64| if v.is_finite() { v.clamp(-1.0, 1.0) } else { 0.0 };
    r[0] = 1.0;
    r[1] = 1.0;
    r[3] = s;
    r[4] = c;
    r[5] = proximity(d, range_m);
    r[6] = cl((t.y_m - a.pose.y_m) / crate::HEIGHT_REFERENCE_M);
    r[7] = rel_heading.sin();
    r[8] = rel_heading.cos();
    r[9] = f64::from(t.moving);
    r[10] = f64::from(t.posture == posture::REST);
    r[11] = f64::from(t.posture == posture::ASLEEP || t.posture == posture::DOZE);
    r[12] = f64::from(t.stance == perception::stance::GUARD);
    r[13] = f64::from(t.stance == perception::stance::RUSH);
    r[14] = f64::from(t.lame);
    r[15] = level(t.size_m, own_length);
    r[16] = if lost_s > 0.0 { (1.0 - age_s / lost_s).clamp(0.0, 1.0) } else { 0.0 };
    r[17] = f64::from(t.modes & perception::SIGHT != 0);
    r[18] = f64::from(t.modes & perception::HEARING != 0);
    r[19] = f64::from(t.modes & perception::TREMOR != 0);
    r[20 + rel] = 1.0;
    r[24] = f64::from(t.class == class::THREAT);
    r[25] = f64::from(t.class == class::PREY);
    r[31] = f64::from(remembered);
    Candidate {
        kind: RowKind::Body,
        subject: Subject::of_track(t),
        row: r,
        kin: rel <= 2,
        threat: t.class == class::THREAT,
        prey: t.class == class::PREY,
        place: None,
    }
}

/// A place row.
fn place_row(frame: &Frame, a: &Animal, kind: PlaceKind, x: f64, z: f64, y_m: f64, seen_tick: u64, range_m: f64, lost_s: f64, remembered: bool) -> Candidate {
    let mut r = [0.0; ROW_WIDTH];
    let (s, c, d) = frame.toward(x, z);
    let age_s = frame.tick.saturating_sub(seen_tick) as f64 * cubarium_voxel::DT;
    r[0] = 1.0;
    r[2] = 1.0;
    r[3] = s;
    r[4] = c;
    r[5] = proximity(d, range_m);
    r[6] = if y_m.is_finite() { ((y_m - a.pose.y_m) / crate::HEIGHT_REFERENCE_M).clamp(-1.0, 1.0) } else { 0.0 };
    r[16] = if lost_s > 0.0 { (1.0 - age_s / lost_s).clamp(0.0, 1.0) } else { 1.0 };
    r[26 + kind.index()] = 1.0;
    r[31] = f64::from(remembered);
    Candidate {
        kind: RowKind::Place,
        subject: Subject::Place { kind, x, z, y_m, seen_tick },
        row: r,
        kin: false,
        threat: false,
        prey: false,
        place: Some(kind),
    }
}

/// **Body `i`'s candidates** (module note) with its held `subject`: `K` rows from its own
/// record and its lineage's slots ([`crate::IntentKnobs`]). A plain lineage has none.
pub fn candidates(fauna: &Fauna, i: usize, founder: Founder, held: Subject) -> Candidates {
    let a = &fauna.animals[i];
    let manifest = fauna.config.manifest(founder);
    let knobs = fauna.config.founder(founder).intent;
    let (nk, nt, np, nplace) = (manifest.kin_slots, manifest.threat_slots, manifest.prey_slots, manifest.place_slots);
    let k_all = nk + nt + np + nplace;
    if k_all == 0 {
        return Candidates::default();
    }
    let ring = fauna.world_geometry.map_or(f64::INFINITY, |(r, _)| r);
    let frame = Frame::of(a, ring, fauna.tick);
    let range = manifest.slot_range_m;
    let lost_s = knobs.subject_lost_s;
    let own_length = fauna.config.founder(founder).body_at(a.body).length_m;
    let fresh = |t: &&Track| t.known && t.age(fauna.tick) as f64 * cubarium_voxel::DT <= lost_s;
    let mut tracks: Vec<Track> = fauna.percepts.get(&a.id).map(|p| p.tracks().iter().filter(fresh).copied().collect()).unwrap_or_default();
    tracks.sort_by(|p, q| p.d_m.total_cmp(&q.d_m).then(p.id.cmp(&q.id)));
    // The held body: kept in its class's rows whatever its rank, and as remembered at its
    // last-known point when the latest sensing lost it but it is not yet given up.
    let mut keep: Option<(Track, bool)> = None;
    if let Subject::Body { id, seen_tick, at } = held {
        if let Some(t) = tracks.iter().find(|t| t.id == id) {
            keep = Some((*t, false));
        } else if let Some([x, z, y_m]) = at {
            let age_s = fauna.tick.saturating_sub(seen_tick) as f64 * cubarium_voxel::DT;
            let alive = fauna.animals.binary_search_by_key(&id, |o| o.id).ok();
            if age_s <= lost_s {
                if let Some(cls) = alive.and_then(|k| perception::class_of(&fauna.config, a, &fauna.animals[k])) {
                    let t = Track { known: true, id, class: cls, x, z, y_m, tick: seen_tick, d_m: frame.toward(x, z).2, ..Track::default() };
                    keep = Some((t, true));
                }
            }
        }
    }
    let mut rows: Vec<Candidate> = Vec::with_capacity(k_all);
    let take = |cls: u8, n: usize, rows: &mut Vec<Candidate>| {
        let mut picked: Vec<(Track, bool)> = tracks.iter().filter(|t| t.class == cls).map(|t| (*t, false)).collect();
        if cls == class::KIN {
            // The recognised mother or calf first.
            if let Some(k) = picked.iter().position(|(t, _)| relation(fauna, a, t) <= 1) {
                let first = picked.remove(k);
                picked.insert(0, first);
            }
        }
        picked.truncate(n);
        if let Some(k) = keep.filter(|(t, _)| t.class == cls && n > 0) {
            if !picked.iter().any(|(t, _)| t.id == k.0.id) {
                if picked.len() == n {
                    picked.pop();
                }
                picked.push(k);
            }
        }
        for (t, rem) in &picked {
            rows.push(body_row(fauna, a, &frame, t, range, lost_s, own_length, *rem));
        }
        rows.extend(std::iter::repeat_n(Candidate::default(), n - picked.len()));
    };
    take(class::KIN, nk, &mut rows);
    take(class::THREAT, nt, &mut rows);
    take(class::PREY, np, &mut rows);
    // Places: the held place, then the lineage's home place it always knows (a pack's den).
    let mut places: Vec<Candidate> = Vec::new();
    if let Subject::Place { kind, x, z, y_m, seen_tick } = held {
        places.push(place_row(&frame, a, kind, x, z, y_m, seen_tick, range, f64::INFINITY, true));
    }
    let v = fauna.world_geometry.map_or(1.0, |(_, v)| v);
    if let Some(p) = a.pack.and_then(|m| fauna.pack(m.pack)) {
        let (x, z) = ((f64::from(p.den.x) + 0.5) * v, (f64::from(p.den.z) + 0.5) * v);
        if !places.iter().any(|c| matches!(c.subject, Subject::Place { kind: PlaceKind::Den, x: px, z: pz, .. } if px == x && pz == z)) {
            places.push(place_row(&frame, a, PlaceKind::Den, x, z, crate::surface_m(p.den.y, v), fauna.tick, range, f64::INFINITY, false));
        }
    }
    places.truncate(nplace);
    let n = places.len();
    rows.extend(places);
    rows.extend(std::iter::repeat_n(Candidate::default(), nplace - n));
    Candidates { rows }
}

/// **One decision sample's inputs beyond the observation** for body `i` (the producer of
/// [`crate::DecisionInput`]): its intent mask, candidates, row masks, the held subject's row
/// and the early-end flag, read from its record before this sample.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecisionPrep {
    pub menu: usize,
    pub intent_mask: Vec<bool>,
    pub candidates: Candidates,
    pub flat: Vec<f64>,
    pub kinds: Vec<RowKind>,
    /// Each row's stable key ([`subject_key`]).
    pub keys: Vec<u64>,
    pub candidate_mask: Vec<bool>,
    pub held_row: Option<usize>,
    pub early_end: Option<crate::EarlyEnd>,
}

impl DecisionPrep {
    /// The borrowed view a controller gets.
    pub fn input(&self) -> crate::DecisionInput<'_> {
        crate::DecisionInput {
            menu: self.menu,
            intent_mask: &self.intent_mask,
            candidates: &self.flat,
            row_width: ROW_WIDTH,
            kinds: &self.kinds,
            keys: &self.keys,
            candidate_mask: &self.candidate_mask,
            held_row: self.held_row,
            early_end: self.early_end,
        }
    }
}

/// Body `i`'s [`DecisionPrep`] (read only).
pub fn decision_prep(fauna: &Fauna, i: usize, founder: Founder) -> DecisionPrep {
    let a = &fauna.animals[i];
    let menu = crate::intent::menu_of(founder);
    let knobs = fauna.config.founder(founder).intent;
    let rec = a.intent;
    let held_subject = crate::intent::check_subject(fauna, a.id, rec.subject, knobs.subject_lost_s);
    let candidates = candidates(fauna, i, founder, held_subject.unwrap_or(Subject::None));
    let young = crate::intent::young(fauna, a, founder);
    let intent_mask = crate::intent::intent_mask(menu, young, |f| candidates.any(f));
    let early_end = match held_subject {
        Err(why) => Some(why),
        Ok(_) if !intent_mask.get(usize::from(rec.held)).copied().unwrap_or(false) => Some(crate::EarlyEnd::Masked),
        Ok(_) if rec.overridden != crate::Override::None => Some(crate::EarlyEnd::Emergency),
        Ok(_) => None,
    };
    let held_row = held_subject.ok().and_then(|s| candidates.row_of(s));
    DecisionPrep {
        menu: menu.len(),
        candidate_mask: candidates.mask(menu),
        flat: candidates.flat(),
        kinds: candidates.rows.iter().map(|c| c.kind).collect(),
        keys: candidates.rows.iter().map(|c| c.key()).collect(),
        intent_mask,
        candidates,
        held_row,
        early_end,
    }
}
