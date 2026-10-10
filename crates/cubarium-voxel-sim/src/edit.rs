//! **WORLD-EDIT: place, remove, pick up and move plants and animals** (Wrysk, 2026-10-09:
//! "ability to place/remove plants and animals in the terrarium at will. and ablity to pick
//! them up and move them … this might even let us just manually seed world scenarios").
//!
//! The one front over the terrain and both layers: it resolves where ([`At`]) and hands the
//! layer its edit — `cubarium_voxel_fauna`'s and `cubarium_voxel_flora`'s `edit` modules
//! hold the validity, social and ledger rules, each in its own module note. Edits apply
//! between ticks only. A refused edit returns its reason and changes nothing.
//!
//! Bulk helpers for studies: [`Edit::thin`] and [`Edit::populate`]. A scenario file
//! ([`Scenario`], TOML) lists edits for a headless run (`voxel_census … edit=FILE.toml`).
//! [`Edit::pick`] lists what stands at or near a point, for a picker.

use cubarium_voxel::World;
use cubarium_voxel::keyed::KeyedRng;
use cubarium_voxel_fauna::{BodySpec, BodyStage, Fauna, Founder, Group, Lifted, Placed, Spot};
use cubarium_voxel_flora::{Flora, LiftedStand, PlantPlaced, PlantStage, Site, Species};
use serde::{Deserialize, Serialize};

/// How far, in cells either way, a point looks for a valid face to snap to. A search window,
/// not an ecological number.
pub const SNAP_CELLS: i64 = 3;

/// The keyed stream the bulk helpers draw from.
const EDIT_DOMAIN: u64 = 0xED17;

/// **Where an edit goes.**
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum At {
    /// Exactly this support face (`y` the solid voxel with open air over it).
    Face { x: u32, y: u32, z: u32 },
    /// The highest support face of column `(x, z)`.
    Column { x: i64, z: u32 },
    /// A world point, metres: the valid face nearest it within [`SNAP_CELLS`]; with `aloft`
    /// (a flyer), exactly that point in the air.
    Point {
        x_m: f64,
        y_m: f64,
        z_m: f64,
        #[serde(default)]
        aloft: bool,
    },
}

/// A rectangle of columns, inclusive; `x` wraps (`x[0] > x[1]` runs across the seam).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub x: [i64; 2],
    pub z: [u32; 2],
}

impl Region {
    fn holds(&self, width: u32, s: Site) -> bool {
        let w = i64::from(width);
        let (a, b) = (self.x[0].rem_euclid(w), self.x[1].rem_euclid(w));
        let x = i64::from(s.x);
        let in_x = if a <= b { (a..=b).contains(&x) } else { x >= a || x <= b };
        in_x && (self.z[0]..=self.z[1]).contains(&s.z)
    }
}

/// Something held off the world.
#[derive(Debug)]
pub enum Held {
    Animal(Lifted),
    Plant(LiftedStand),
}

/// What [`Edit::thin`] did.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Thinned {
    pub before: usize,
    pub after: usize,
    /// The ids removed.
    pub removed: Vec<u64>,
    /// Whole groups removed (packs, or a loftstrider adult with its calves).
    pub groups_removed: usize,
    /// The group trimmed to land exactly on the count, and how many it lost (young first).
    pub trimmed: Option<(String, usize)>,
}

/// What [`Edit::populate`] did.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Populated {
    pub placed: Vec<Placed>,
    /// Why the shortfall, when fewer than asked were placed: the last refusal seen.
    pub shortfall: Option<String>,
}

/// One animal near a point ([`Edit::pick`]).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PickedAnimal {
    pub id: u64,
    pub lineage: &'static str,
    pub stage: &'static str,
    pub site: Site,
    pub x_m: f64,
    pub y_m: f64,
    pub z_m: f64,
    pub distance_m: f64,
    pub pack: Option<u64>,
    pub rider_of: Option<u64>,
}

/// One plant near a point.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PickedPlant {
    pub id: u64,
    pub species: &'static str,
    pub place: &'static str,
    pub site: Site,
    pub wood: f64,
    pub distance_m: f64,
}

/// One latticevine rooted near a point.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PickedVine {
    pub id: u64,
    pub root: Site,
    pub faces: usize,
    pub distance_m: f64,
}

/// What stands at or near a point, nearest first.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Picked {
    pub animals: Vec<PickedAnimal>,
    pub plants: Vec<PickedPlant>,
    pub vines: Vec<PickedVine>,
}

/// What a cover paint takes: a stand species (springturf, or any) or the latticevine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cover {
    Stand(Species),
    Latticevine,
}

/// A lineage by its name (`frondgrazer`), its role (`browser`) or `veilhound`.
pub fn lineage_named(s: &str) -> Option<Founder> {
    let s = s.trim().to_ascii_lowercase();
    if s == "veilhound" {
        return Some(Founder::Chorister);
    }
    Founder::ALL.into_iter().find(|f| f.name() == s || f.role() == s || format!("{f:?}").to_ascii_lowercase() == s)
}

/// The edit API over a world and its two layers, borrowed for the edits.
pub struct Edit<'a> {
    pub world: &'a World,
    pub flora: &'a mut Flora,
    pub fauna: &'a mut Fauna,
}

fn face_of(world: &World, x: u32, y: u32, z: u32) -> Site {
    let w = world.config().width;
    Site { x: x % w.max(1), y, z }
}

impl<'a> Edit<'a> {
    pub fn new(world: &'a World, flora: &'a mut Flora, fauna: &'a mut Fauna) -> Edit<'a> {
        Edit { world, flora, fauna }
    }

    /// The faces near a point, nearest first.
    fn snap_candidates(&self, x_m: f64, y_m: f64, z_m: f64) -> Vec<Site> {
        let view = self.world.view();
        let c = view.config;
        let v = c.voxel_m;
        let (cx, cz) = ((x_m / v).floor() as i64, (z_m / v).floor() as i64);
        let ring = f64::from(c.width) * v;
        let mut out: Vec<(f64, Site)> = Vec::new();
        for dz in -SNAP_CELLS..=SNAP_CELLS {
            let z = cz + dz;
            if z < 0 || z >= i64::from(c.depth) {
                continue;
            }
            for dx in -SNAP_CELLS..=SNAP_CELLS {
                let x = (cx + dx).rem_euclid(i64::from(c.width));
                for y in view.supports_in_column(x, z as u32) {
                    let s = Site { x: x as u32, y, z: z as u32 };
                    let (fx, fy, fz) = ((f64::from(s.x) + 0.5) * v, f64::from(s.y + 1) * v, (f64::from(s.z) + 0.5) * v);
                    let ddx = (fx - x_m).rem_euclid(ring);
                    let ddx = ddx.min(ring - ddx);
                    let d = (ddx * ddx + (fy - y_m).powi(2) + (fz - z_m).powi(2)).sqrt();
                    out.push((d, s));
                }
            }
        }
        out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        out.into_iter().map(|(_, s)| s).collect()
    }

    /// The support face `at` names (a point: every face near it, nearest first), or why none.
    fn faces_for(&self, at: At) -> Result<Vec<Site>, String> {
        match at {
            At::Face { x, y, z } => Ok(vec![face_of(self.world, x, y, z)]),
            At::Column { x, z } => cubarium_voxel_flora::highest_support(&self.world.view(), x, z)
                .map(|s| vec![s])
                .ok_or_else(|| format!("column ({x}, {z}) has no support face")),
            At::Point { x_m, y_m, z_m, .. } => {
                let c = self.snap_candidates(x_m, y_m, z_m);
                if c.is_empty() { Err("no support face near that point".into()) } else { Ok(c) }
            }
        }
    }

    /// The first of `faces` `ok` takes, or the first face's refusal.
    fn first_ok<T>(faces: Vec<Site>, mut ok: impl FnMut(Site) -> Result<T, String>) -> Result<T, String> {
        let mut first: Option<String> = None;
        for s in faces {
            match ok(s) {
                Ok(t) => return Ok(t),
                Err(e) => {
                    first.get_or_insert(e);
                }
            }
        }
        Err(first.unwrap_or_else(|| "no face".into()))
    }

    // ---------------------------------------------------------------- animals

    /// **Place an animal** of `founder` at `at` as `spec` says.
    pub fn place_animal(&mut self, founder: Founder, at: At, spec: BodySpec) -> Result<Placed, String> {
        if let At::Point { x_m, y_m, z_m, aloft: true } = at {
            let pose = cubarium_voxel_fauna::Pose { x: x_m, z: z_m, y_m, heading_rad: spec.heading_rad };
            return self.fauna.place_body(self.world, self.flora, founder, Spot::Aloft(pose), spec);
        }
        let faces = self.faces_for(at)?;
        let (world, flora, fauna) = (self.world, &*self.flora, &mut *self.fauna);
        Self::first_ok(faces, |s| fauna.place_body(world, flora, founder, Spot::Face(s), spec))
    }

    /// **Remove animal `id`**: no carcass; booked removed.
    pub fn remove_animal(&mut self, id: u64) -> Result<(), String> {
        self.fauna.remove_body(id)
    }

    /// **Lift animal `id`** off the world, its whole state kept.
    pub fn lift_animal(&mut self, id: u64) -> Result<Lifted, String> {
        self.fauna.lift_body(id)
    }

    /// **Set a lifted animal down at `at`**; refused with it handed back.
    pub fn set_down_animal(&mut self, held: Lifted, at: At) -> Result<u64, (Lifted, String)> {
        let founder = held.animal().founder;
        let body = held.animal().body;
        let spot = match at {
            At::Point { x_m, y_m, z_m, aloft: true } => {
                Spot::Aloft(cubarium_voxel_fauna::Pose { x: x_m, z: z_m, y_m, heading_rad: held.animal().pose.heading_rad })
            }
            _ => {
                let faces = match self.faces_for(at) {
                    Ok(f) => f,
                    Err(e) => return Err((held, e)),
                };
                let Some(f) = founder else { return Err((held, "not a founder body".into())) };
                match Self::first_ok(faces, |s| self.fauna.check_spot(self.world, f, body, Spot::Face(s)).map(|()| s)) {
                    Ok(s) => Spot::Face(s),
                    Err(e) => return Err((held, e)),
                }
            }
        };
        self.fauna.set_down_body(self.world, held, spot)
    }

    /// **Move animal `id` to `at`**: its state, riders and group kept; books nothing.
    pub fn move_animal(&mut self, id: u64, at: At) -> Result<(), String> {
        if let At::Point { x_m, y_m, z_m, aloft: true } = at {
            let h = self.fauna.view().animal(id).map_or(0.0, |a| a.pose.heading_rad);
            let pose = cubarium_voxel_fauna::Pose { x: x_m, z: z_m, y_m, heading_rad: h };
            return self.fauna.move_body(self.world, id, Spot::Aloft(pose));
        }
        let faces = self.faces_for(at)?;
        let (world, fauna) = (self.world, &mut *self.fauna);
        Self::first_ok(faces, |s| fauna.move_body(world, id, Spot::Face(s)))
    }

    // ----------------------------------------------------------------- plants

    /// **Place a `species` plant** at `at`, at `stage`.
    pub fn place_plant(&mut self, species: Species, at: At, stage: PlantStage) -> Result<PlantPlaced, String> {
        let faces = self.faces_for(at)?;
        let (world, flora) = (self.world, &mut *self.flora);
        Self::first_ok(faces, |s| flora.place_plant(world, species, s, stage))
    }

    /// **Remove plant `id`**; booked removed.
    pub fn remove_plant(&mut self, id: u64) -> Result<(), String> {
        self.flora.remove_stand(self.world, id)
    }

    /// **Lift plant `id`** off the world, its state kept.
    pub fn lift_plant(&mut self, id: u64) -> Result<LiftedStand, String> {
        self.flora.lift_stand(self.world, id)
    }

    /// **Set a lifted plant down at `at`** (a transplant).
    pub fn set_down_plant(&mut self, held: LiftedStand, at: At) -> Result<u64, (LiftedStand, String)> {
        let faces = match self.faces_for(at) {
            Ok(f) => f,
            Err(e) => return Err((held, e)),
        };
        let mut held = held;
        let mut first: Option<String> = None;
        for s in faces {
            match self.flora.set_down_stand(self.world, held, s) {
                Ok(id) => return Ok(id),
                Err((h, e)) => {
                    held = h;
                    first.get_or_insert(e);
                }
            }
        }
        Err((held, first.unwrap_or_else(|| "no face".into())))
    }

    /// **Move plant `id` to `at`** (a transplant in place); books nothing.
    pub fn move_plant(&mut self, id: u64, at: At) -> Result<(), String> {
        let faces = self.faces_for(at)?;
        let (world, flora) = (self.world, &mut *self.flora);
        Self::first_ok(faces, |s| flora.move_stand(world, id, s))
    }

    /// The support faces in `region` (every face when `None`), low to high.
    pub fn faces_in(&self, region: Option<Region>) -> Vec<Site> {
        let w = self.world.config().width;
        let mut all = crate::habitat::support_sites(self.world);
        if let Some(r) = region {
            all.retain(|s| r.holds(w, *s));
        }
        all
    }

    /// **Paint cover** over `sites` at `amount` (`0..=1`: a stand species' share of its
    /// `wood_max`, a latticevine face's leafiness). The ids made, and the refusals.
    pub fn paint(&mut self, cover: Cover, sites: &[Site], amount: f64) -> (Vec<u64>, Vec<(Site, String)>) {
        match cover {
            Cover::Stand(species) => self.flora.paint(self.world, species, sites, amount),
            Cover::Latticevine => {
                let (mut done, mut refused) = (Vec::new(), Vec::new());
                for &s in sites {
                    match self.flora.paint_vine(self.world, s, amount) {
                        Ok(id) => done.push(id),
                        Err(e) => refused.push((s, e)),
                    }
                }
                (done, refused)
            }
        }
    }

    /// **Clear** every stand and rooted latticevine on `sites`: `(stands, vines)`.
    pub fn clear(&mut self, sites: &[Site]) -> (usize, usize) {
        self.flora.clear_sites(self.world, sites)
    }

    // ------------------------------------------------------------ bulk helpers

    /// **Thin `founder` to `keep` bodies**, chosen at random by `seed`, groups kept whole:
    /// - a pack is one group (its pups with it); a loftstrider-style herd (a lineage whose
    ///   herding pulls only its young) is every body joined by chains within its herding
    ///   `comfort_m` ([`cubarium_voxel_fauna::herd_labels`], as dispersal counts it); every
    ///   other body is its own group;
    /// - whole groups go, in a shuffled order, while that leaves at least `keep`;
    /// - then, to land exactly on `keep`, the next group loses members — young first (the
    ///   youngest first), then adults other than a pack's breeder — so it keeps at least one
    ///   adult and no young is left without one.
    ///
    /// A host removed frees its riders (they are their own lineage's bodies). Eggs are not
    /// bodies and stay.
    pub fn thin(&mut self, founder: Founder, keep: usize, seed: u64) -> Thinned {
        let phys = *self.fauna.config().founder(founder);
        let birth = phys.core.birth_body;
        let v = self.world.config().voxel_m;
        let ring = f64::from(self.world.config().width) * v;
        let bodies: Vec<cubarium_voxel_fauna::Animal> =
            self.fauna.view().animals.iter().filter(|a| a.founder == Some(founder)).copied().collect();
        let before = bodies.len();
        let mut out = Thinned { before, after: before, ..Thinned::default() };
        if before <= keep {
            return out;
        }
        // The groups, each with its label and trim order (young youngest first, then adults,
        // the breeder last).
        let mut groups: Vec<(String, Vec<u64>)> = Vec::new();
        let mut grouped: std::collections::BTreeMap<u64, usize> = std::collections::BTreeMap::new();
        let young = |a: &cubarium_voxel_fauna::Animal| a.body < birth;
        if phys.pack.is_some() {
            for p in self.fauna.packs().iter().filter(|p| p.lineage == founder) {
                let mut m: Vec<&cubarium_voxel_fauna::Animal> =
                    bodies.iter().filter(|a| a.pack.is_some_and(|k| k.pack == p.id)).collect();
                if m.is_empty() {
                    continue;
                }
                m.sort_by_key(|a| (!young(a), Some(a.id) == p.breeder, if young(a) { a.age_ticks } else { u64::MAX - a.age_ticks }, a.id));
                for a in &m {
                    grouped.insert(a.id, groups.len());
                }
                groups.push((format!("pack {}", p.id), m.iter().map(|a| a.id).collect()));
            }
        } else if phys.herding.young_only && phys.herding.comfort_m > 0.0 {
            // The herd, as dispersal counts it: chains within comfort_m, x wrapping.
            let refs: Vec<&cubarium_voxel_fauna::Animal> = bodies.iter().collect();
            let herd = cubarium_voxel_fauna::herd_labels(&refs, phys.herding.comfort_m, ring);
            for (k, a) in bodies.iter().enumerate() {
                if herd[k] != k {
                    continue;
                }
                let mut h: Vec<&cubarium_voxel_fauna::Animal> =
                    bodies.iter().zip(&herd).filter(|(_, l)| **l == k).map(|(b, _)| b).collect();
                if h.len() < 2 {
                    continue;
                }
                // Young youngest first, then adults youngest first.
                h.sort_by_key(|b| (!young(b), b.age_ticks, b.id));
                for b in &h {
                    grouped.insert(b.id, groups.len());
                }
                groups.push((format!("herd of {}", a.id), h.iter().map(|b| b.id).collect()));
            }
        }
        for a in &bodies {
            if !grouped.contains_key(&a.id) {
                groups.push((format!("body {}", a.id), vec![a.id]));
            }
        }
        // A shuffled order (Fisher–Yates on the keyed stream).
        let mut rng = KeyedRng::keyed(EDIT_DOMAIN, seed, founder.index() as u64, before as u64);
        for i in (1..groups.len()).rev() {
            let j = rng.below(i + 1);
            groups.swap(i, j);
        }
        let mut left = before;
        for (label, members) in groups {
            if left == keep {
                break;
            }
            let take = if left - members.len() >= keep {
                if members.len() > 1 {
                    out.groups_removed += 1;
                }
                members.len()
            } else {
                let n = left - keep;
                out.trimmed = Some((label, n));
                n
            };
            for &id in members.iter().take(take) {
                if self.fauna.remove_body(id).is_ok() {
                    out.removed.push(id);
                }
            }
            left -= take;
        }
        out.after = self.fauna.view().animals.iter().filter(|a| a.founder == Some(founder)).count();
        out
    }

    /// **Add `n` bodies of `founder`** at valid faces in `region` (anywhere when `None`),
    /// spread out: faces in a shuffled order (by `seed`), each taken only beyond half the even
    /// spacing (`½ √(area / n)`) from those already placed, then the rest of the faces to
    /// fill. A pack lineage's adults come in packs of its `pack_size` on one face each (the
    /// first founds it, the rest join); every other body comes as `spec` says.
    pub fn populate(&mut self, founder: Founder, n: usize, seed: u64, region: Option<Region>, spec: BodySpec) -> Populated {
        let mut out = Populated::default();
        if n == 0 {
            return out;
        }
        let mut faces = self.faces_in(region);
        let c = self.world.config().clone();
        let v = c.voxel_m;
        let ring = f64::from(c.width) * v;
        let mut rng = KeyedRng::keyed(EDIT_DOMAIN, seed, founder.index() as u64 | 1 << 32, n as u64);
        for i in (1..faces.len()).rev() {
            let j = rng.below(i + 1);
            faces.swap(i, j);
        }
        let mut columns: Vec<(u32, u32)> = faces.iter().map(|s| (s.x, s.z)).collect();
        columns.sort_unstable();
        columns.dedup();
        let area = columns.len() as f64 * c.cell_area();
        let pack_size = self
            .fauna
            .config()
            .founder(founder)
            .pack
            .filter(|_| spec.stage == BodyStage::Adult)
            .map(|t| t.pack_size.max(1));
        let units = pack_size.map_or(n, |k| n.div_ceil(k));
        let spacing = 0.5 * (area / units as f64).sqrt();
        let mut taken: Vec<Site> = Vec::new();
        let far = |taken: &[Site], s: Site| {
            taken.iter().all(|t| {
                let dx = (f64::from(t.x) - f64::from(s.x)).abs() * v;
                let dx = dx.min(ring - dx);
                dx.hypot((f64::from(t.z) - f64::from(s.z)) * v) >= spacing
            })
        };
        let mut placed = 0usize;
        for pass in 0..2 {
            for &s in &faces {
                if placed >= n {
                    break;
                }
                if taken.contains(&s) || (pass == 0 && !far(&taken, s)) {
                    continue;
                }
                let k = pack_size.map_or(1, |k| k.min(n - placed));
                let first = BodySpec { group: if pack_size.is_some() { Group::New } else { spec.group }, ..spec };
                match self.fauna.place_body(self.world, self.flora, founder, Spot::Face(s), first) {
                    Ok(p) => {
                        taken.push(s);
                        placed += 1;
                        out.placed.push(p);
                        if let Placed::Body { pack: Some(pack), .. } = p {
                            for _ in 1..k {
                                let join = BodySpec { group: Group::Join(pack), ..spec };
                                match self.fauna.place_body(self.world, self.flora, founder, Spot::Face(s), join) {
                                    Ok(q) => {
                                        placed += 1;
                                        out.placed.push(q);
                                    }
                                    Err(e) => out.shortfall = Some(e),
                                }
                            }
                        }
                    }
                    Err(e) => out.shortfall = Some(e),
                }
            }
        }
        if placed >= n {
            out.shortfall = None;
        } else if out.shortfall.is_none() {
            out.shortfall = Some("no face left in the region".into());
        }
        out
    }

    /// **What stands within `radius_m` of a point**, nearest first: animals by their pose,
    /// plants and vine roots by their face's top centre.
    pub fn pick(&self, x_m: f64, y_m: f64, z_m: f64, radius_m: f64) -> Picked {
        let c = self.world.config();
        let v = c.voxel_m;
        let ring = f64::from(c.width) * v;
        let dist = |x: f64, y: f64, z: f64| {
            let dx = (x - x_m).rem_euclid(ring);
            let dx = dx.min(ring - dx);
            (dx * dx + (y - y_m).powi(2) + (z - z_m).powi(2)).sqrt()
        };
        let top = |s: Site| ((f64::from(s.x) + 0.5) * v, f64::from(s.y + 1) * v, (f64::from(s.z) + 0.5) * v);
        let mut out = Picked::default();
        let fv = self.fauna.view();
        for a in fv.animals {
            let d = dist(a.pose.x, a.pose.y_m, a.pose.z);
            if d > radius_m {
                continue;
            }
            let Some(f) = a.founder else { continue };
            let birth = self.fauna.config().founder(f).core.birth_body;
            out.animals.push(PickedAnimal {
                id: a.id,
                lineage: f.name(),
                stage: if a.attached.is_some() {
                    "rider"
                } else if a.body < birth {
                    "juvenile"
                } else {
                    "adult"
                },
                site: a.site,
                x_m: a.pose.x,
                y_m: a.pose.y_m,
                z_m: a.pose.z,
                distance_m: d,
                pack: a.pack.map(|m| m.pack),
                rider_of: a.attached.map(|h| h.host),
            });
        }
        let pv = self.flora.view();
        for s in pv.stands {
            let (x, y, z) = top(s.site);
            let d = dist(x, y, z);
            if d <= radius_m {
                out.plants.push(PickedPlant {
                    id: s.id,
                    species: s.species.name(),
                    place: s.place.name(),
                    site: s.site,
                    wood: s.wood,
                    distance_m: d,
                });
            }
        }
        for vine in pv.cover.vines() {
            let (x, y, z) = top(vine.root);
            let d = dist(x, y, z);
            if d <= radius_m {
                out.vines.push(PickedVine { id: vine.id, root: vine.root, faces: pv.cover.faces_of(vine.id).len(), distance_m: d });
            }
        }
        out.animals.sort_by(|a, b| a.distance_m.total_cmp(&b.distance_m).then(a.id.cmp(&b.id)));
        out.plants.sort_by(|a, b| a.distance_m.total_cmp(&b.distance_m).then(a.id.cmp(&b.id)));
        out.vines.sort_by(|a, b| a.distance_m.total_cmp(&b.distance_m).then(a.id.cmp(&b.id)));
        out
    }
}

impl crate::Sim {
    /// Run `f` over the edit API on this world, between ticks.
    pub fn edit<R>(&mut self, f: impl FnOnce(&mut Edit<'_>) -> R) -> R {
        self.with_layers_mut(|world, flora, fauna| f(&mut Edit::new(world, flora, fauna)))
    }
}

// ------------------------------------------------------------------ scenario files

/// **A scenario file** (`voxel_census … edit=FILE.toml`): edits applied after founding or
/// after `load=`, before the first tick. Applied in this order whatever order the file
/// lists them in — every removal before any addition: `[[remove]]`, `[[clear]]`, `[[thin]]`,
/// `[[paint]]`, `[[place]]`, `[[populate]]`; within a kind, in file order.
///
/// ```toml
/// [[thin]]
/// lineage = "frondgrazer"   # a lineage's name or role, or "veilhound"
/// keep = 50
/// seed = 7
///
/// [[populate]]
/// lineage = "loftstrider"
/// count = 4
/// seed = 1
/// region = { x = [0, 40], z = [0, 20] }   # optional; x wraps
/// stage = "adult"                         # optional: egg | juvenile | adult | rider
/// fullness = 0.8                          # optional
///
/// [[place]]
/// lineage = "chorister"                   # or `species = "vaulttree"` for a plant
/// at = { x = 10, z = 5 }                  # a column; { x, y, z } a face; { x_m, y_m, z_m } a point
/// stage = "adult"                         # plants: seed | seedling | mature
/// group = "auto"                          # "auto" | "new" | a pack id to join
///
/// [[remove]]
/// animal = 12                             # or `plant = 40`
///
/// [[paint]]
/// species = "springturf"                  # or "latticevine"
/// region = { x = [0, 10], z = [0, 10] }   # or faces = [[x, y, z], ...]
/// amount = 0.5
///
/// [[clear]]
/// region = { x = [0, 10], z = [0, 10] }   # or faces = [[x, y, z], ...]
/// ```
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    #[serde(default)]
    pub remove: Vec<RemoveEntry>,
    #[serde(default)]
    pub clear: Vec<AreaEntry>,
    #[serde(default)]
    pub thin: Vec<ThinEntry>,
    #[serde(default)]
    pub paint: Vec<PaintEntry>,
    #[serde(default)]
    pub place: Vec<PlaceEntry>,
    #[serde(default)]
    pub populate: Vec<PopulateEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoveEntry {
    pub animal: Option<u64>,
    pub plant: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaEntry {
    pub region: Option<Region>,
    pub faces: Option<Vec<[u32; 3]>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThinEntry {
    pub lineage: String,
    pub keep: usize,
    #[serde(default)]
    pub seed: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaintEntry {
    pub species: String,
    pub region: Option<Region>,
    pub faces: Option<Vec<[u32; 3]>>,
    pub amount: f64,
}

/// A pack to join, or `"auto"` / `"new"`.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum GroupEntry {
    Join(u64),
    Named(String),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaceEntry {
    pub lineage: Option<String>,
    pub species: Option<String>,
    pub at: At,
    pub stage: Option<String>,
    pub fullness: Option<f64>,
    pub group: Option<GroupEntry>,
    #[serde(default)]
    pub heading: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopulateEntry {
    pub lineage: String,
    pub count: usize,
    #[serde(default)]
    pub seed: u64,
    pub region: Option<Region>,
    pub stage: Option<String>,
    pub fullness: Option<f64>,
}

/// What a scenario did: one line per entry applied, one per entry (or face) refused.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ScenarioReport {
    pub applied: Vec<String>,
    pub refused: Vec<String>,
}

impl Scenario {
    pub fn from_toml(text: &str) -> Result<Scenario, String> {
        toml::from_str(text).map_err(|e| format!("scenario: {e}"))
    }

    pub fn read(path: &std::path::Path) -> Result<Scenario, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("scenario {}: {e}", path.display()))?;
        Scenario::from_toml(&text)
    }

    /// Apply every entry (the order in the type's note), and say what happened.
    pub fn apply(&self, e: &mut Edit<'_>) -> ScenarioReport {
        let mut r = ScenarioReport::default();
        let sites = |e: &Edit<'_>, region: Option<Region>, faces: &Option<Vec<[u32; 3]>>| -> Vec<Site> {
            match faces {
                Some(f) => f.iter().map(|[x, y, z]| face_of(e.world, *x, *y, *z)).collect(),
                None => e.faces_in(region),
            }
        };
        for (k, x) in self.remove.iter().enumerate() {
            let res = match (x.animal, x.plant) {
                (Some(id), None) => e.remove_animal(id).map(|()| format!("removed animal {id}")),
                (None, Some(id)) => e.remove_plant(id).map(|()| format!("removed plant {id}")),
                _ => Err("wants exactly one of `animal` or `plant`".into()),
            };
            push(&mut r, format!("remove[{k}]"), res);
        }
        for (k, x) in self.clear.iter().enumerate() {
            let s = sites(e, x.region, &x.faces);
            let (stands, vines) = e.clear(&s);
            r.applied.push(format!("clear[{k}]: {stands} plants and {vines} vines off {} faces", s.len()));
        }
        for (k, x) in self.thin.iter().enumerate() {
            let res = lineage_named(&x.lineage).ok_or_else(|| format!("no lineage {:?}", x.lineage)).map(|f| {
                let t = e.thin(f, x.keep, x.seed);
                format!(
                    "thinned {} {} -> {} ({} whole groups{})",
                    f.name(),
                    t.before,
                    t.after,
                    t.groups_removed,
                    t.trimmed.map_or(String::new(), |(g, n)| format!(", {g} trimmed by {n}"))
                )
            });
            push(&mut r, format!("thin[{k}]"), res);
        }
        for (k, x) in self.paint.iter().enumerate() {
            let cover = if x.species.eq_ignore_ascii_case("latticevine") {
                Some(Cover::Latticevine)
            } else {
                Species::parse(&x.species).map(Cover::Stand)
            };
            let Some(cover) = cover else {
                r.refused.push(format!("paint[{k}]: no plant {:?}", x.species));
                continue;
            };
            let s = sites(e, x.region, &x.faces);
            let (done, refused) = e.paint(cover, &s, x.amount);
            r.applied.push(format!("paint[{k}]: {} {} on {} of {} faces", x.species, x.amount, done.len(), s.len()));
            for (why, n) in tally(refused.iter().map(|(_, w)| w.clone())) {
                r.refused.push(format!("paint[{k}]: {n} faces: {why}"));
            }
        }
        for (k, x) in self.place.iter().enumerate() {
            let res = place_entry(e, x);
            push(&mut r, format!("place[{k}]"), res);
        }
        for (k, x) in self.populate.iter().enumerate() {
            let res = (|| {
                let f = lineage_named(&x.lineage).ok_or_else(|| format!("no lineage {:?}", x.lineage))?;
                let stage = match &x.stage {
                    Some(s) => BodyStage::parse(s).ok_or_else(|| format!("no animal stage {s:?}"))?,
                    None => BodyStage::Adult,
                };
                let spec = BodySpec { stage, fullness: x.fullness, ..BodySpec::default() };
                let p = e.populate(f, x.count, x.seed, x.region, spec);
                Ok::<_, String>(format!(
                    "populated {} {}/{}{}",
                    f.name(),
                    p.placed.len(),
                    x.count,
                    p.shortfall.map_or(String::new(), |w| format!(" (short: {w})"))
                ))
            })();
            push(&mut r, format!("populate[{k}]"), res);
        }
        r
    }
}

fn push(r: &mut ScenarioReport, what: String, res: Result<String, String>) {
    match res {
        Ok(s) => r.applied.push(format!("{what}: {s}")),
        Err(e) => r.refused.push(format!("{what}: {e}")),
    }
}

/// Reasons and how often each came, in first-seen order.
fn tally(reasons: impl Iterator<Item = String>) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for w in reasons {
        match out.iter_mut().find(|(r, _)| *r == w) {
            Some((_, n)) => *n += 1,
            None => out.push((w, 1)),
        }
    }
    out
}

/// One `[[place]]` entry.
pub fn place_entry(e: &mut Edit<'_>, x: &PlaceEntry) -> Result<String, String> {
    match (&x.lineage, &x.species) {
        (Some(l), None) => {
            let f = lineage_named(l).ok_or_else(|| format!("no lineage {l:?}"))?;
            let stage = match &x.stage {
                Some(s) => BodyStage::parse(s).ok_or_else(|| format!("no animal stage {s:?}"))?,
                None => BodyStage::Adult,
            };
            let group = match &x.group {
                None => Group::Auto,
                Some(GroupEntry::Join(id)) => Group::Join(*id),
                Some(GroupEntry::Named(s)) if s == "auto" => Group::Auto,
                Some(GroupEntry::Named(s)) if s == "new" => Group::New,
                Some(GroupEntry::Named(s)) => return Err(format!("group {s:?}: auto, new or a pack id")),
            };
            let spec = BodySpec { stage, fullness: x.fullness, group, heading_rad: x.heading };
            e.place_animal(f, x.at, spec).map(|p| match p {
                Placed::Body { id, pack } => format!("{} {id}{}", f.name(), pack.map_or(String::new(), |p| format!(" in pack {p}"))),
                Placed::Eggs { site, eggs } => format!("{eggs} {} egg at {site:?}", f.name()),
            })
        }
        (None, Some(sp)) => {
            let species = Species::parse(sp).ok_or_else(|| format!("no plant {sp:?}"))?;
            let stage = match &x.stage {
                Some(s) => PlantStage::parse(s).ok_or_else(|| format!("no plant stage {s:?}"))?,
                None => PlantStage::Mature,
            };
            e.place_plant(species, x.at, stage).map(|p| match p {
                PlantPlaced::Stand { id } => format!("{} {id}", species.name()),
                PlantPlaced::Seeds { site, .. } => format!("{} seed at {site:?}", species.name()),
            })
        }
        _ => Err("wants exactly one of `lineage` or `species`".into()),
    }
}

// ------------------------------------------------------------------ JSON commands

/// **What a running world holds off itself**, by token: the session keeps one per run. A
/// save while something is held saves copies back at their lift spots
/// ([`HeldTable::restore_copies`]).
#[derive(Debug, Default)]
pub struct HeldTable {
    next: u64,
    items: std::collections::BTreeMap<u64, Held>,
}

impl HeldTable {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    fn hold(&mut self, h: Held) -> u64 {
        let t = self.next;
        self.next += 1;
        self.items.insert(t, h);
        t
    }

    /// Copies of everything held put back at their lift spots, into layers about to be
    /// saved (clones of the running ones).
    pub fn restore_copies(&self, flora: &mut Flora, fauna: &mut Fauna) {
        for h in self.items.values() {
            match h {
                Held::Animal(a) => fauna.restore_copy(a),
                Held::Plant(p) => flora.restore_stand_copy(p),
            }
        }
    }

    /// Everything held put back at its lift spot (a world closing or replaced).
    pub fn restore_all(&mut self, e: &mut Edit<'_>) {
        for (_, h) in std::mem::take(&mut self.items) {
            match h {
                Held::Animal(a) => {
                    e.fauna.restore(a);
                }
                Held::Plant(p) => {
                    e.flora.restore_stand(e.world, p);
                }
            }
        }
    }

    fn list(&self) -> serde_json::Value {
        let rows: Vec<serde_json::Value> = self
            .items
            .iter()
            .map(|(t, h)| match h {
                Held::Animal(a) => serde_json::json!({
                    "held": t, "kind": "animal", "id": a.id(),
                    "lineage": a.animal().founder.map(|f| f.name()), "riders": a.riders().len(), "from": a.from_site(),
                }),
                Held::Plant(p) => serde_json::json!({
                    "held": t, "kind": "plant", "id": p.id(), "species": p.stand().species.name(), "from": p.stand().site,
                }),
            })
            .collect();
        serde_json::Value::Array(rows)
    }
}

/// One edit as JSON (the session's `edit` command and line): `{"op": …, …}`.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum EditCommand {
    PlaceAnimal {
        lineage: String,
        at: At,
        stage: Option<String>,
        fullness: Option<f64>,
        group: Option<GroupEntry>,
        #[serde(default)]
        heading: f64,
    },
    RemoveAnimal { id: u64 },
    LiftAnimal { id: u64 },
    MoveAnimal { id: u64, at: At },
    PlacePlant { species: String, at: At, stage: Option<String> },
    RemovePlant { id: u64 },
    LiftPlant { id: u64 },
    MovePlant { id: u64, at: At },
    /// Set down whatever token `held` names.
    SetDown { held: u64, at: At },
    Paint { species: String, region: Option<Region>, faces: Option<Vec<[u32; 3]>>, amount: f64 },
    Clear { region: Option<Region>, faces: Option<Vec<[u32; 3]>> },
    Thin { lineage: String, keep: usize, #[serde(default)] seed: u64 },
    Populate {
        lineage: String,
        count: usize,
        #[serde(default)]
        seed: u64,
        region: Option<Region>,
        stage: Option<String>,
        fullness: Option<f64>,
    },
    Pick { x_m: f64, y_m: f64, z_m: f64, #[serde(default = "pick_radius")] radius_m: f64 },
    Held,
}

fn pick_radius() -> f64 {
    1.0
}

fn refused(why: impl Into<String>) -> serde_json::Value {
    serde_json::json!({ "ok": false, "refused": why.into() })
}

fn ok(mut v: serde_json::Value) -> serde_json::Value {
    if let Some(m) = v.as_object_mut() {
        m.insert("ok".into(), serde_json::Value::Bool(true));
    }
    v
}

/// **Apply one JSON edit** to a world: `{"ok": true, …}` with the ids made, or
/// `{"ok": false, "refused": reason}` with nothing changed.
pub fn command(e: &mut Edit<'_>, held: &mut HeldTable, cmd: &serde_json::Value) -> serde_json::Value {
    let cmd: EditCommand = match serde_json::from_value(cmd.clone()) {
        Ok(c) => c,
        Err(err) => return refused(format!("not an edit: {err}")),
    };
    match cmd {
        EditCommand::PlaceAnimal { lineage, at, stage, fullness, group, heading } => {
            let entry = PlaceEntry { lineage: Some(lineage), species: None, at, stage, fullness, group, heading };
            place_json(e, &entry)
        }
        EditCommand::PlacePlant { species, at, stage } => {
            let entry = PlaceEntry { lineage: None, species: Some(species), at, stage, fullness: None, group: None, heading: 0.0 };
            place_json(e, &entry)
        }
        EditCommand::RemoveAnimal { id } => match e.remove_animal(id) {
            Ok(()) => ok(serde_json::json!({ "removed": id })),
            Err(w) => refused(w),
        },
        EditCommand::RemovePlant { id } => match e.remove_plant(id) {
            Ok(()) => ok(serde_json::json!({ "removed": id })),
            Err(w) => refused(w),
        },
        EditCommand::LiftAnimal { id } => match e.lift_animal(id) {
            Ok(l) => {
                let riders: Vec<u64> = l.riders().iter().map(|r| r.id).collect();
                let t = held.hold(Held::Animal(l));
                ok(serde_json::json!({ "held": t, "id": id, "riders": riders }))
            }
            Err(w) => refused(w),
        },
        EditCommand::LiftPlant { id } => match e.lift_plant(id) {
            Ok(l) => {
                let t = held.hold(Held::Plant(l));
                ok(serde_json::json!({ "held": t, "id": id }))
            }
            Err(w) => refused(w),
        },
        EditCommand::SetDown { held: t, at } => {
            let Some(h) = held.items.remove(&t) else {
                return refused(format!("nothing is held as {t}"));
            };
            match h {
                Held::Animal(l) => match e.set_down_animal(l, at) {
                    Ok(id) => ok(serde_json::json!({ "id": id })),
                    Err((l, w)) => {
                        held.items.insert(t, Held::Animal(l));
                        refused(w)
                    }
                },
                Held::Plant(l) => match e.set_down_plant(l, at) {
                    Ok(id) => ok(serde_json::json!({ "id": id })),
                    Err((l, w)) => {
                        held.items.insert(t, Held::Plant(l));
                        refused(w)
                    }
                },
            }
        }
        EditCommand::MoveAnimal { id, at } => match e.move_animal(id, at) {
            Ok(()) => ok(serde_json::json!({ "id": id, "site": e.fauna.view().animal(id).map(|a| a.site) })),
            Err(w) => refused(w),
        },
        EditCommand::MovePlant { id, at } => match e.move_plant(id, at) {
            Ok(()) => ok(serde_json::json!({ "id": id })),
            Err(w) => refused(w),
        },
        EditCommand::Paint { species, region, faces, amount } => {
            let cover = if species.eq_ignore_ascii_case("latticevine") {
                Some(Cover::Latticevine)
            } else {
                Species::parse(&species).map(Cover::Stand)
            };
            let Some(cover) = cover else { return refused(format!("no plant {species:?}")) };
            let sites = area(e, region, &faces);
            let (done, no) = e.paint(cover, &sites, amount);
            let why: Vec<serde_json::Value> =
                tally(no.into_iter().map(|(_, w)| w)).into_iter().map(|(w, n)| serde_json::json!({ "faces": n, "why": w })).collect();
            ok(serde_json::json!({ "ids": done, "refused_faces": why }))
        }
        EditCommand::Clear { region, faces } => {
            let sites = area(e, region, &faces);
            let (stands, vines) = e.clear(&sites);
            ok(serde_json::json!({ "plants": stands, "vines": vines }))
        }
        EditCommand::Thin { lineage, keep, seed } => match lineage_named(&lineage) {
            Some(f) => ok(serde_json::json!({ "thinned": e.thin(f, keep, seed) })),
            None => refused(format!("no lineage {lineage:?}")),
        },
        EditCommand::Populate { lineage, count, seed, region, stage, fullness } => {
            let Some(f) = lineage_named(&lineage) else { return refused(format!("no lineage {lineage:?}")) };
            let stage = match stage.as_deref().map(BodyStage::parse) {
                None => BodyStage::Adult,
                Some(Some(s)) => s,
                Some(None) => return refused(format!("no animal stage {stage:?}")),
            };
            let p = e.populate(f, count, seed, region, BodySpec { stage, fullness, ..BodySpec::default() });
            ok(serde_json::json!({ "populated": p }))
        }
        EditCommand::Pick { x_m, y_m, z_m, radius_m } => ok(serde_json::json!({ "picked": e.pick(x_m, y_m, z_m, radius_m) })),
        EditCommand::Held => ok(serde_json::json!({ "held": held.list() })),
    }
}

fn area(e: &Edit<'_>, region: Option<Region>, faces: &Option<Vec<[u32; 3]>>) -> Vec<Site> {
    match faces {
        Some(f) => f.iter().map(|[x, y, z]| face_of(e.world, *x, *y, *z)).collect(),
        None => e.faces_in(region),
    }
}

fn place_json(e: &mut Edit<'_>, entry: &PlaceEntry) -> serde_json::Value {
    match (&entry.lineage, &entry.species) {
        (Some(l), None) => {
            let Some(f) = lineage_named(l) else { return refused(format!("no lineage {l:?}")) };
            let stage = match entry.stage.as_deref().map(BodyStage::parse) {
                None => BodyStage::Adult,
                Some(Some(s)) => s,
                Some(None) => return refused(format!("no animal stage {:?}", entry.stage)),
            };
            let group = match &entry.group {
                None => Group::Auto,
                Some(GroupEntry::Join(id)) => Group::Join(*id),
                Some(GroupEntry::Named(s)) if s == "auto" => Group::Auto,
                Some(GroupEntry::Named(s)) if s == "new" => Group::New,
                Some(GroupEntry::Named(s)) => return refused(format!("group {s:?}: auto, new or a pack id")),
            };
            let spec = BodySpec { stage, fullness: entry.fullness, group, heading_rad: entry.heading };
            match e.place_animal(f, entry.at, spec) {
                Ok(p) => ok(serde_json::json!({ "placed": p })),
                Err(w) => refused(w),
            }
        }
        (None, Some(sp)) => {
            let Some(species) = Species::parse(sp) else { return refused(format!("no plant {sp:?}")) };
            let stage = match entry.stage.as_deref().map(PlantStage::parse) {
                None => PlantStage::Mature,
                Some(Some(s)) => s,
                Some(None) => return refused(format!("no plant stage {:?}", entry.stage)),
            };
            match e.place_plant(species, entry.at, stage) {
                Ok(p) => ok(serde_json::json!({ "placed": p })),
                Err(w) => refused(w),
            }
        }
        _ => refused("wants exactly one of `lineage` or `species`"),
    }
}
