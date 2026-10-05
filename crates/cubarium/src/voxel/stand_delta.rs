//! The change-only stand rebuild ([`Stands::set_incremental`]).
//!
//! Everything that stamps cells — a stand, a ground mark (log, litter, film, carrion,
//! seed), the vine's plain cells — is a **claimant** holding the parts it would place, its
//! **claims**, exactly as the full rebuild's [`Stands::place`] calls would place them. A
//! claimant is restamped only when what it draws from moved:
//!
//! - a stand with a baked model: its model step, anchor, wilt level, lip, ripeness and,
//!   per foliage layer, the band of fill within which the same cells are kept
//!   ([`ModelSig`]): the stand's raw stocks move every tick, its drawn cells rarely;
//! - a stand without one ([`parts_of`]): every rebuild, since its colours follow its fill;
//! - a ground mark: its presence and, for a seed or film mark, its species or level;
//! - the vine: a key over what [`vine::cells`] reads of each covered face.
//!
//! A cell's part is the claim the full rebuild would leave there: the highest rank, and of
//! equal ranks the one placed last — claimants in the full rebuild's order (stands by
//! site, then logs, litter, film, carrion and seeds by site, then the vine), claims in
//! their own order. A vine claim ranks below every other, as the full rebuild draws the
//! vine only into cells nothing else took. Only the cells some claimant's claims moved on
//! are resolved again; they are [`Stands::dirty`].
//!
//! A style index names one thing for as long as the rebuild keeps it ([`StyleId`]): a model
//! cell key, a stand without a model, a vine density and accent, a mark kind. That is the
//! full rebuild's equivalence — two crown cells continue one canopy exactly when their
//! style indices are equal — kept across rebuilds.

use cubarium_voxel_flora::Site;
use rustc_hash::FxHashMap;

use crate::sink::gpu::pack_timer::lap;

use super::*;

/// What one style index names in the change-only rebuild, so a style keeps its index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum StyleId {
    /// A stand drawn without a model: its own colours, which follow its fill and wilt.
    Stand(u64),
    /// The vine's plain cells at one density and accent.
    Vine(u8),
    Log,
    Litter,
    Carrion,
    /// A seed mark in one species' colours.
    Seed(u8),
    /// A film mark at one level.
    Film(u8),
}

/// The order a mark kind places in, after every stand; the vine is last.
const LOGS: u64 = 1;
const LITTER: u64 = 2;
const FILM: u64 = 3;
const CARRION: u64 = 4;
const SEEDS: u64 = 5;
const VINES: u64 = 6;

/// A site's order within its pass: `Site`'s own order, `(x, y, z)`.
fn site_key(site: Site) -> u64 {
    (u64::from(site.x) << 32) | (u64::from(site.y) << 16) | u64::from(site.z)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Stand,
    Mark,
    Vines,
}

/// The band of a fraction within which a model keeps the same cells: `lo < f ≤ hi`, for
/// the fraction as [`model::keeps`] reads it ([`effective`]).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Band {
    lo: f64,
    hi: f64,
}

impl Band {
    const ANY: Band = Band {
        lo: f64::NEG_INFINITY,
        hi: f64::INFINITY,
    };

    /// The band around `f` among `thresholds` (a cell is kept iff its threshold `< f`).
    fn around(f: f64, thresholds: impl Iterator<Item = f64>) -> Band {
        let mut b = Band::ANY;
        for t in thresholds {
            if t < f {
                b.lo = b.lo.max(t);
            } else {
                b.hi = b.hi.min(t);
            }
        }
        b
    }

    #[inline]
    fn holds(self, f: f64) -> bool {
        self.lo < f && f <= self.hi
    }
}

/// A fraction as [`model::keeps`] reads it: nothing at or below 0 (or NaN), everything at
/// or above 1, else a cell whose threshold is below it.
#[inline]
fn effective(f: f64) -> f64 {
    if !(f > 0.0) {
        0.0
    } else if f >= 1.0 {
        1.0
    } else {
        f
    }
}

/// What a model stand's stamp read, as far as its cells can tell.
#[derive(Clone, Debug, PartialEq)]
struct ModelSig {
    model: (usize, usize),
    anchor: (i64, u32, u32),
    wilt: u8,
    lip: colours::Lip,
    ripe_at: bool,
    foliage: [Band; MAX_FOLIAGE_LAYERS],
    fruit: Band,
}

impl ModelSig {
    fn of(cells: &[ModelCell], id: u64, inp: &ModelInputs) -> ModelSig {
        let mut foliage = [Band::ANY; MAX_FOLIAGE_LAYERS];
        for (i, band) in foliage.iter_mut().enumerate() {
            let f = effective(inp.foliage[i]);
            *band = Band::around(
                f,
                cells.iter().filter_map(|m| match m.tag {
                    Tag::Foliage(l) | Tag::Drape(l) if usize::from(l) == i => {
                        Some(model::keep_threshold(id, m.offset, l))
                    }
                    _ => None,
                }),
            );
        }
        let fruit = if inp.fruit {
            Band::around(
                effective(inp.ripe),
                cells
                    .iter()
                    .filter(|m| m.tag == Tag::Accent && m.material == model::WARM)
                    .map(|m| model::keep_threshold(id, m.offset, model::FRUIT_LAYER)),
            )
        } else {
            Band::ANY
        };
        ModelSig {
            model: (cells.as_ptr() as usize, cells.len()),
            anchor: (inp.anchor.x, inp.anchor.y, inp.anchor.z),
            wilt: inp.wilt,
            lip: inp.lip,
            ripe_at: inp.ripe >= model::RIPE_AT,
            foliage,
            fruit,
        }
    }

    /// Whether a stamp of `cells` from `inp` would place what this one did.
    fn holds(&self, cells: &[ModelCell], inp: &ModelInputs) -> bool {
        self.model == (cells.as_ptr() as usize, cells.len())
            && self.anchor == (inp.anchor.x, inp.anchor.y, inp.anchor.z)
            && self.wilt == inp.wilt
            && self.lip == inp.lip
            && self.ripe_at == (inp.ripe >= model::RIPE_AT)
            && self
                .foliage
                .iter()
                .zip(&inp.foliage)
                .all(|(b, &f)| b.holds(effective(f)))
            && (!inp.fruit || self.fruit.holds(effective(inp.ripe)))
    }
}

struct Claimant {
    kind: Kind,
    order: u64,
    /// The rebuild that last saw it.
    seen: u64,
    claims: Vec<(u32, Part)>,
    sig: Option<ModelSig>,
}

/// The change-only rebuild's state.
#[derive(Default)]
pub(crate) struct Inc {
    claimants: Vec<Claimant>,
    free: Vec<u32>,
    by_stand: FxHashMap<u64, u32>,
    marks: FxHashMap<(u64, Site), u32>,
    vines: Option<u32>,
    vine_key: Option<u64>,
    /// Per grid cell with any claim: `(claimant, claim index)`.
    cell_claims: FxHashMap<u32, Vec<(u32, u32)>>,
    pub(crate) style_ids: FxHashMap<StyleId, u16>,
    /// Set when [`Stands::new_style`] changed a kept style's colours.
    pub(crate) restyled: bool,
    /// Each stamped cell's place in `Stands::stamped`.
    stamp_pos: FxHashMap<u32, u32>,
    /// Each stand claimant's index in this rebuild's `FloraView::stands`.
    pub(crate) k_of_slot: Vec<u32>,
    seen: u64,
    /// Cells some claim moved on this rebuild.
    changed: Vec<u32>,
    pub(crate) dirty: Vec<u32>,
    /// Each ground site's marks last rebuild: log, litter, carrion, seed species + 1.
    ground: Vec<(Site, [u8; 4])>,
    /// Each film mark's site and level last rebuild.
    film: Vec<(Site, u8)>,
    pub(crate) reset: bool,
    started: bool,
}

/// One change-only rebuild.
pub(super) fn rebuild(s: &mut Stands, view: &VoxelView<'_>, flora: FloraView<'_>, lib: Option<&ModelLibrary>) {
    let mut inc = s.inc.take().expect("an incremental rebuild");
    inc.dirty.clear();
    inc.changed.clear();
    if inc.reset || !inc.started {
        // From nothing: every cell stamped so far is dirty, every claimant goes.
        for &i in &s.stamped {
            s.grid[i as usize] = Part::None;
            if !s.owner.is_empty() {
                s.owner[i as usize] = 0;
            }
        }
        inc.dirty.extend(s.stamped.drain(..));
        let started = std::mem::take(&mut inc.k_of_slot);
        drop(started);
        *inc = Inc {
            started: true,
            dirty: std::mem::take(&mut inc.dirty),
            ..Inc::default()
        };
        s.styles.clear();
        s.model_tags.clear();
        s.emits.clear();
        s.model_styles.fill(NO_STYLE);
    }
    inc.seen += 1;
    inc.restyled = false;
    s.inc = Some(inc);

    stands(s, view, flora, lib);
    lap!("stand keys");
    marks(s, view, flora);
    lap!("marks");
    if s.vine_cells {
        vines(s, view, flora);
    }
    resolve(s);

    // A style table grown far past what is live (stands without a model renaming their
    // colours) starts again next rebuild.
    let inc = s.inc.as_mut().expect("kept");
    if s.styles.len() > 32_768 {
        inc.reset = true;
    }
}

fn inc(s: &mut Stands) -> &mut Inc {
    s.inc.as_mut().expect("an incremental rebuild")
}

/// A claimant slot, new or reused.
fn claimant(inc: &mut Inc, kind: Kind, order: u64) -> u32 {
    let c = Claimant {
        kind,
        order,
        seen: inc.seen,
        claims: Vec::new(),
        sig: None,
    };
    match inc.free.pop() {
        Some(slot) => {
            inc.claimants[slot as usize] = c;
            slot
        }
        None => {
            inc.claimants.push(c);
            (inc.claimants.len() - 1) as u32
        }
    }
}

/// Give `slot` the claims `new`; the cells either set touched are re-resolved. With
/// `force`, the cells are re-resolved even when the claims did not move (a style they
/// name changed its colours).
fn set_claims(inc: &mut Inc, slot: u32, new: Vec<(u32, Part)>, force: bool) {
    let old = std::mem::take(&mut inc.claimants[slot as usize].claims);
    if old == new && !force {
        inc.claimants[slot as usize].claims = old;
        return;
    }
    for &(cell, _) in &old {
        if let Some(list) = inc.cell_claims.get_mut(&cell) {
            list.retain(|&(s, _)| s != slot);
            if list.is_empty() {
                inc.cell_claims.remove(&cell);
            }
        }
        inc.changed.push(cell);
    }
    for (k, &(cell, _)) in new.iter().enumerate() {
        inc.cell_claims.entry(cell).or_default().push((slot, k as u32));
        inc.changed.push(cell);
    }
    inc.claimants[slot as usize].claims = new;
}

fn drop_claimant(inc: &mut Inc, slot: u32) {
    set_claims(inc, slot, Vec::new(), false);
    inc.free.push(slot);
}

/// Collect what `stamp` places, as claims.
fn collect(s: &mut Stands, stamp: impl FnOnce(&mut Stands)) -> Vec<(u32, Part)> {
    s.collect = Some(Vec::new());
    stamp(s);
    s.collect.take().unwrap_or_default()
}

fn stands(s: &mut Stands, view: &VoxelView<'_>, flora: FloraView<'_>, lib: Option<&ModelLibrary>) {
    let seen = inc(s).seen;
    for (k, stand) in flora.stands.iter().enumerate() {
        // A face's plant before its fungus, as the full rebuild places them.
        let order = (site_key(stand.site) << 1) | flora.config.face_layer(stand.species).index() as u64;
        let moved = as_drawn(view, flora, stand);
        let stand = moved.as_ref().unwrap_or(stand);
        let i = inc(s);
        let slot = match i.by_stand.get(&stand.id) {
            Some(&slot) => slot,
            None => {
                let slot = claimant(i, Kind::Stand, order);
                i.by_stand.insert(stand.id, slot);
                slot
            }
        };
        if i.k_of_slot.len() <= slot as usize {
            i.k_of_slot.resize(slot as usize + 1, 0);
        }
        i.k_of_slot[slot as usize] = k as u32;
        let c = &mut i.claimants[slot as usize];
        c.seen = seen;
        c.order = order;
        s.placing = slot + 1;
        match lib.and_then(|lib| model_cells(lib, flora, stand)) {
            Some(cells) => {
                let inp = s.model_inputs(flora, stand);
                let held = inc(s).claimants[slot as usize]
                    .sig
                    .as_ref()
                    .is_some_and(|sig| sig.holds(cells, &inp));
                if held {
                    continue;
                }
                let claims = collect(s, |s| {
                    s.stamp_model(view, flora, stand, cells);
                });
                let i = inc(s);
                i.claimants[slot as usize].sig = Some(ModelSig::of(cells, stand.id, &inp));
                set_claims(i, slot, claims, false);
            }
            None => {
                inc(s).restyled = false;
                let style = s.new_style(StyleId::Stand(stand.id), style_of(flora, stand));
                let claims = match style {
                    Some(style) => collect(s, |s| {
                        for (cell, part) in parts_of(flora, stand, style) {
                            s.place(view, cell, part);
                        }
                    }),
                    None => Vec::new(),
                };
                let i = inc(s);
                let force = i.restyled;
                i.claimants[slot as usize].sig = None;
                set_claims(i, slot, claims, force);
            }
        }
    }
    s.placing = 0;
    let i = inc(s);
    let gone: Vec<(u64, u32)> = i
        .by_stand
        .iter()
        .filter(|(_, slot)| i.claimants[**slot as usize].seen != seen)
        .map(|(&id, &slot)| (id, slot))
        .collect();
    for (id, slot) in gone {
        i.by_stand.remove(&id);
        drop_claimant(i, slot);
    }
}

/// Set the mark of pass `pass` at `site` to `part_of(style)` in `style`, or remove it.
fn mark(
    s: &mut Stands,
    view: &VoxelView<'_>,
    pass: u64,
    site: Site,
    style: Option<(StyleId, Style)>,
    part_of: fn(u16) -> Part,
) {
    let key = (pass, site);
    match style {
        None => {
            let i = inc(s);
            if let Some(slot) = i.marks.remove(&key) {
                drop_claimant(i, slot);
            }
        }
        Some((id, value)) => {
            let Some(style) = s.new_style(id, value) else { return };
            let claims = collect(s, |s| {
                s.place(
                    view,
                    Cell {
                        x: i64::from(site.x),
                        y: site.y + 1,
                        z: site.z,
                    },
                    part_of(style),
                );
            });
            let i = inc(s);
            let slot = match i.marks.get(&key) {
                Some(&slot) => slot,
                None => {
                    let slot = claimant(i, Kind::Mark, (pass << 48) | site_key(site));
                    i.marks.insert(key, slot);
                    slot
                }
            };
            set_claims(i, slot, claims, false);
        }
    }
}

fn marks(s: &mut Stands, view: &VoxelView<'_>, flora: FloraView<'_>) {
    // Logs, litter, carrion and seed marks: each ground site's state against last
    // rebuild's, both in site order.
    let old = std::mem::take(&mut inc(s).ground);
    let mut new: Vec<(Site, [u8; 4])> = Vec::with_capacity(flora.ground.len());
    for g in flora.ground {
        let seed = g
            .seed_mark(flora.tick, flora.config)
            .map_or(0, |sp| sp.index() as u8 + 1);
        new.push((
            g.site,
            [
                u8::from(g.dead_wood >= 0.05),
                u8::from(g.litter >= 0.05),
                u8::from(g.carrion >= 0.05),
                seed,
            ],
        ));
    }
    let mut changes: Vec<(Site, [u8; 4], [u8; 4])> = Vec::new();
    {
        let (mut a, mut b) = (old.iter().peekable(), new.iter().peekable());
        loop {
            match (a.peek(), b.peek()) {
                (None, None) => break,
                (Some(&&(sa, va)), Some(&&(sb, vb))) if sa == sb => {
                    if va != vb {
                        changes.push((sa, va, vb));
                    }
                    a.next();
                    b.next();
                }
                (Some(&&(sa, va)), Some(&&(sb, _))) if sa < sb => {
                    changes.push((sa, va, [0; 4]));
                    a.next();
                }
                (Some(&&(sa, va)), None) => {
                    changes.push((sa, va, [0; 4]));
                    a.next();
                }
                (_, Some(&&(sb, vb))) => {
                    changes.push((sb, [0; 4], vb));
                    b.next();
                }
            }
        }
    }
    inc(s).ground = new;
    for (site, was, now) in changes {
        let passes: [(usize, u64, StyleId, fn() -> Style, fn(u16) -> Part); 3] = [
            (0, LOGS, StyleId::Log, log_style, Part::Log),
            (1, LITTER, StyleId::Litter, litter_style, Part::Litter),
            (2, CARRION, StyleId::Carrion, carrion_style, Part::Carrion),
        ];
        for (k, pass, id, style, part) in passes {
            if was[k] != now[k] {
                mark(s, view, pass, site, (now[k] != 0).then(|| (id, style())), part);
            }
        }
        if was[3] != now[3] {
            let style = now[3]
                .checked_sub(1)
                .and_then(|i| Species::ALL.get(usize::from(i)))
                .map(|&sp| (StyleId::Seed(sp.index() as u8), seed_style(sp)));
            mark(s, view, SEEDS, site, style, Part::Sprout);
        }
    }

    lap!("ground marks");
    // Film: every drawn mark, at its level.
    s.ease_film_marks(flora);
    lap!("film ease");
    let old = std::mem::take(&mut inc(s).film);
    let new: Vec<(Site, u8)> = s.film_drawn.iter().map(|&(site, d)| (site, film_level(d))).collect();
    let (mut a, mut b) = (old.iter().peekable(), new.iter().peekable());
    let mut changes: Vec<(Site, Option<u8>)> = Vec::new();
    loop {
        match (a.peek(), b.peek()) {
            (None, None) => break,
            (Some(&&(sa, la)), Some(&&(sb, lb))) if sa == sb => {
                if la != lb {
                    changes.push((sb, Some(lb)));
                }
                a.next();
                b.next();
            }
            (Some(&&(sa, _)), Some(&&(sb, _))) if sa < sb => {
                changes.push((sa, None));
                a.next();
            }
            (Some(&&(sa, _)), None) => {
                changes.push((sa, None));
                a.next();
            }
            (_, Some(&&(sb, lb))) => {
                changes.push((sb, Some(lb)));
                b.next();
            }
        }
    }
    inc(s).film = new;
    for (site, level) in changes {
        let style = level.map(|l| (StyleId::Film(l), film_style(f32::from(l) / FILM_LEVELS)));
        mark(s, view, FILM, site, style, Part::Film);
    }
}

/// What [`vine::cells`] reads of the covered faces, as one key: the same faces drawing the
/// same way give the same key whatever order the cover holds them in.
pub(crate) fn vine_key(draws: impl Iterator<Item = FaceDraw>, root: impl Fn(u64) -> Option<Site>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut sum = 0u64;
    let mut n = 0u64;
    for d in draws {
        let mut h = rustc_hash::FxHasher::default();
        (d.face.x, d.face.y, d.face.z, d.face.dir as u8).hash(&mut h);
        (vine::density(d.leafiness, d.dormant) as u8, vine::accent(d.spur) as u8, d.rooted).hash(&mut h);
        if d.rooted {
            root(d.owner).map(|r| (r.x, r.y, r.z)).hash(&mut h);
        }
        sum = sum.wrapping_add(h.finish());
        n += 1;
    }
    sum ^ n.rotate_left(32)
}

fn vines(s: &mut Stands, view: &VoxelView<'_>, flora: FloraView<'_>) {
    let key = match &s.cover_override {
        Some(d) => vine_key(d.iter().copied(), |id| flora.cover.vine(id).map(|v| v.root)),
        None => vine_key(flora.cover.face_draws(), |id| flora.cover.vine(id).map(|v| v.root)),
    };
    if inc(s).vine_key == Some(key) {
        return;
    }
    inc(s).restyled = false;
    let claims = collect(s, |s| s.place_vines(view, flora));
    let i = inc(s);
    i.vine_key = Some(key);
    let force = i.restyled;
    let slot = match i.vines {
        Some(slot) => slot,
        None => {
            let slot = claimant(i, Kind::Vines, VINES << 48);
            i.vines = Some(slot);
            slot
        }
    };
    set_claims(i, slot, claims, force);
}

/// Resolve every cell some claim moved on: its part, owner and place in `stamped`.
fn resolve(s: &mut Stands) {
    let mut inc = s.inc.take().expect("an incremental rebuild");
    let rank = |kind: Kind, p: Part| -> u8 {
        if kind == Kind::Vines {
            return 1;
        }
        match p {
            Part::None => 0,
            Part::Litter(_) | Part::Carrion(_) | Part::Film(_) => 2,
            Part::Sprout(_) => 3,
            Part::Log(_) => 4,
            Part::Crown { .. } => 5,
            Part::Trunk(_) => 6,
        }
    };
    let mut changed = std::mem::take(&mut inc.changed);
    changed.sort_unstable();
    changed.dedup();
    for &cell in &changed {
        let mut best: Option<((u8, u64, u32), Part, u32)> = None;
        if let Some(list) = inc.cell_claims.get(&cell) {
            for &(slot, k) in list {
                let c = &inc.claimants[slot as usize];
                let part = c.claims[k as usize].1;
                let r = rank(c.kind, part);
                if r == 0 {
                    continue;
                }
                let at = (r, c.order, k);
                if best.is_none_or(|(b, _, _)| at > b) {
                    let owner = if c.kind == Kind::Stand { slot + 1 } else { 0 };
                    best = Some((at, part, owner));
                }
            }
        }
        let i = cell as usize;
        let (part, owner) = best.map_or((Part::None, 0), |(_, p, o)| (p, o));
        s.grid[i] = part;
        if !s.owner.is_empty() {
            s.owner[i] = owner;
        }
        match (part != Part::None, inc.stamp_pos.get(&cell).copied()) {
            (true, None) => {
                inc.stamp_pos.insert(cell, s.stamped.len() as u32);
                s.stamped.push(cell);
            }
            (false, Some(at)) => {
                inc.stamp_pos.remove(&cell);
                s.stamped.swap_remove(at as usize);
                if let Some(&moved) = s.stamped.get(at as usize) {
                    inc.stamp_pos.insert(moved, at);
                }
            }
            _ => {}
        }
    }
    inc.dirty.extend_from_slice(&changed);
    inc.changed = changed;
    s.inc = Some(inc);
}
