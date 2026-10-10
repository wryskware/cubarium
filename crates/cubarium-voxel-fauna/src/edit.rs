//! **WORLD-EDIT: animals placed, removed, lifted and set down at will** (Wrysk, 2026-10-09:
//! "ability to place/remove plants and animals in the terrarium at will. and ablity to pick
//! them up and move them"). The animal half of the edit API; `cubarium_voxel_sim::edit` is
//! the whole-world front that resolves a spot and calls these.
//!
//! Edits run between ticks only, like [`Fauna::apply`]. A refused edit leaves the layer
//! untouched: every check runs before anything changes.
//!
//! **Validity** reuses the rules the layer already runs:
//! - a walker (and a glider on the ground) needs the founding faces' clearance: water no
//!   deeper than it wades ([`FounderPhysiology::wade_depth_m`]), its whole disc clear of
//!   solid over its headroom ([`crate::disc_clear_on_face`]), and its centre no nearer the
//!   strip's `z` ends than its radius;
//! - a water-bound body (the ripple-snail) needs a face its step rule admits (standing water
//!   within its margin, or wet enough ground on land) and the same clearance;
//! - a flyer needs room on the face ([`crate::flyer_fits_on_face`]) and water under its drown
//!   depth; in the air, the introduction's own placement rule (`flight::placement`);
//! - eggs go where the lineage lays: a dormant egg is buried on any support face
//!   ([`Command::BuryClutch`]); another lineage's egg needs its [`crate::EggSite`];
//! - a rider (the lanternjaw's larva) needs a host by the hatching rule: the nearest living
//!   body not of its lineage and not itself riding, within `parasite_attach_radius_m`, with
//!   fewer than `parasite_max_per_host` riders.
//!
//! **Social ties.** Only a pack lineage keeps a group record ([`PackRecord`]); the herds,
//! troops and the loftstrider calf's family are kin each body senses, with nothing to join.
//! - `Group::Auto` on a pack lineage joins the nearest pack of the lineage whose den is within
//!   its `territory_radius_m`; with none, an adult founds a new pack (its den the face it is
//!   placed on) and a juvenile joins the nearest pack of the lineage anywhere (refused when
//!   there is none: a pup needs a pack).
//! - `Group::New` founds a pack on the placed face with the body its breeder (adults only);
//!   `Group::Join(id)` joins that pack (same lineage only). A juvenile joins as a pup.
//! - On a lineage with no group record, `Auto` is accepted and `New`/`Join` refused.
//! - Removing or lifting a member runs the pack's succession at once (a missing breeder
//!   gives way to the oldest adult; a pack with no member left is dropped). A lift that empties
//!   a pack keeps its record with the body, and setting the body down restores it.
//! - Lifting a host carries its riders; lifting a rider detaches it (booked `riders_freed`).
//!   Removing a host frees its riders where it stood, as [`Command::Remove`] does.
//!
//! **Ledger.** A placed body or egg is booked `introduced_*_in` (and counted `introduced` /
//! `eggs_buried`); a removed one `removed_*_out` with departure cause `Removed`. A lift books
//! what leaves as `removed_*_out` and a set-down books it back as `introduced_*_in`, so the
//! layer's books close while a body is held; neither is a birth or a departure. A move
//! relocates the body in place and books nothing: no matter crosses the boundary.

use cubarium_voxel::{VoxelView, World};
use cubarium_voxel_flora::{Flora, Site};
use serde::{Deserialize, Serialize};

use crate::body::WaterRule;
use crate::{
    Animal, Attachment, BirthMode, Clutch, Controller, Founder, MobilityMode, PackMember, PackRecord,
    Percept, Pose, Season, Species,
};
use crate::{Fauna, flight};

/// The life stage a placed animal arrives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyStage {
    /// One egg of the lineage (a clutch of one): dormant and buried for an egg-bank lineage,
    /// incubating otherwise. Refused for a lineage that bears live young.
    Egg,
    /// A juvenile at the lineage's `body_min`, a newborn's size.
    Juvenile,
    /// A grown body: the founding arrival's structure (`founding.arrival_body` of
    /// `body_max`), and at least the lineage's `birth_body`.
    Adult,
    /// **The lanternjaw's rider larva**: a juvenile attached to a host near the spot (the
    /// hatching rule's host). Only for a lineage whose eggs wait in the bank and hatch onto a
    /// host.
    Rider,
}

impl BodyStage {
    pub fn parse(s: &str) -> Option<BodyStage> {
        match s {
            "egg" => Some(BodyStage::Egg),
            "juvenile" | "young" => Some(BodyStage::Juvenile),
            "adult" => Some(BodyStage::Adult),
            "rider" | "larva" => Some(BodyStage::Rider),
            _ => None,
        }
    }
}

/// Which group a placed body belongs to (module note).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    #[default]
    Auto,
    New,
    Join(u64),
}

/// How a placed body arrives.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BodySpec {
    pub stage: BodyStage,
    /// Its reserve as a fraction of what its structure carries, and its stomach as the same
    /// fraction of its capacity, `0..=1`. `None`: the founding arrival
    /// (`founding.arrival_reserve` and `founding.arrival_stomach`).
    pub fullness: Option<f64>,
    pub group: Group,
    pub heading_rad: f64,
}

impl Default for BodySpec {
    fn default() -> BodySpec {
        BodySpec { stage: BodyStage::Adult, fullness: None, group: Group::Auto, heading_rad: 0.0 }
    }
}

/// Where a body goes: on a support face, or (a flyer only) at an exact pose in the air.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spot {
    Face(Site),
    Aloft(Pose),
}

/// What a placement made.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Placed {
    Body { id: u64, pack: Option<u64> },
    Eggs { site: Site, eggs: u32 },
}

/// **A body held off the world** ([`Fauna::lift_body`]): its whole state, its riders, its
/// controller and its percept and place records, and its pack's record when the lift
/// emptied the pack. It does not step. Not saved: a save while it is held saves a copy back
/// at its lift spot ([`Fauna::restore_copy`]).
pub struct Lifted {
    animal: Animal,
    riders: Vec<Animal>,
    controllers: Vec<(u64, Box<dyn Controller>)>,
    percepts: Vec<(u64, Percept)>,
    memory: Vec<(u64, crate::step::places::Memory)>,
    pack: Option<PackRecord>,
}

impl std::fmt::Debug for Lifted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lifted")
            .field("id", &self.animal.id)
            .field("founder", &self.animal.founder)
            .field("riders", &self.riders.iter().map(|r| r.id).collect::<Vec<_>>())
            .field("pack", &self.pack.map(|p| p.id))
            .finish()
    }
}

impl Lifted {
    pub fn id(&self) -> u64 {
        self.animal.id
    }
    /// The body as it was lifted (at its lift spot).
    pub fn animal(&self) -> &Animal {
        &self.animal
    }
    /// The riders carried with it.
    pub fn riders(&self) -> &[Animal] {
        &self.riders
    }
    /// Where it was lifted from.
    pub fn from_site(&self) -> Site {
        self.animal.site
    }
}

/// Where a body would stand: its support face, pose and mode.
#[derive(Clone, Copy, Debug)]
struct Stance {
    site: Site,
    pose: Pose,
    mode: MobilityMode,
    perch: Option<crate::Perch>,
}

impl Fauna {
    fn index_of(&self, id: u64) -> Option<usize> {
        self.animals.binary_search_by_key(&id, |a| a.id).ok()
    }

    /// The structure a body of `founder` arrives with at `stage`.
    fn stage_body(&self, founder: Founder, stage: BodyStage) -> f64 {
        let core = self.config.founder(founder).core;
        match stage {
            BodyStage::Juvenile | BodyStage::Rider | BodyStage::Egg => core.body_min,
            BodyStage::Adult => {
                (self.config.founding.arrival_body * core.body_max).max(core.birth_body).min(core.body_max).max(core.body_min)
            }
        }
    }

    /// **Whether a body of `founder` holding `body` of structure could be at `spot`**
    /// (module note), and how it would stand there; `Err` says why not.
    pub fn check_spot(&self, world: &World, founder: Founder, body: f64, spot: Spot) -> Result<(), String> {
        self.stance(&world.view(), founder, body, spot, 0.0).map(|_| ())
    }

    fn stance(&self, view: &VoxelView<'_>, founder: Founder, body: f64, spot: Spot, heading: f64) -> Result<Stance, String> {
        let phys = *self.config.founder(founder);
        let c = view.config;
        let v = c.voxel_m;
        match spot {
            Spot::Aloft(pose) => {
                let Some(fl) = phys.flight else {
                    return Err(format!("a {} cannot fly: place it on a face", founder.name()));
                };
                let (site, perch) = flight::placement(view, &phys, &fl, body, &pose)
                    .ok_or("no room in the air there (inside rock or water, no support below, or above its ceiling)")?;
                let mode = if perch.is_some() { MobilityMode::Perched } else { MobilityMode::Fly };
                Ok(Stance { site, pose, mode, perch })
            }
            Spot::Face(site) => {
                if site.x >= c.width || site.z >= c.depth || site.y + 1 >= c.height {
                    return Err("outside the world".into());
                }
                let (x, y, z) = (i64::from(site.x), site.y, site.z);
                if !view.is_support(x, y, z) {
                    return Err("not a support face (solid with open air over it)".into());
                }
                let shape = phys.body_at(body);
                let depth = view.water_depth_m(x, y, z);
                let pose = Pose { heading_rad: heading, ..Pose::at_site(site, v) };
                if phys.flight.is_some() {
                    if depth > phys.drown_depth_m(&shape) {
                        return Err(format!("under {depth:.2} m of water, deeper than a {} survives", founder.name()));
                    }
                    if !flight::flyer_fits_on_face(view, &phys, body, site, 0.0) {
                        return Err("no room for its wings on that face".into());
                    }
                    let perch = crate::Perch { kind: crate::PerchKind::Face, anchor: crate::PerchAnchor::Face(site) };
                    return Ok(Stance { site, pose, mode: MobilityMode::Perched, perch: Some(perch) });
                }
                if let Some(rule) = WaterRule::of(&phys) {
                    if !rule.admits(view, x, y, z) {
                        return Err(format!("a {} needs water there (or wet ground by it)", founder.name()));
                    }
                } else if depth > phys.wade_depth_m(&shape) {
                    return Err(format!("water {depth:.2} m deep, deeper than a {} wades", founder.name()));
                }
                let r = shape.footprint_radius();
                let zc = (f64::from(site.z) + 0.5) * v;
                if zc + 1e-9 < r || zc > f64::from(c.depth) * v - r + 1e-9 {
                    return Err("too near the world's edge for its body".into());
                }
                if !crate::disc_clear_on_face(view, &shape, site, phys.step_limits(v)) {
                    return Err(format!("no room for a {}'s body there (rock or a low roof)", founder.name()));
                }
                Ok(Stance { site, pose, mode: MobilityMode::Ground, perch: None })
            }
        }
    }

    /// The pack a body of `founder` at `stage` placed on `site` would belong to, by its
    /// group: `Ok(None)` for no pack, `Ok(Some(None))` to found a new one, `Ok(Some(Some(id)))`
    /// to join `id`.
    fn resolve_group(
        &self,
        view: &VoxelView<'_>,
        founder: Founder,
        stage: BodyStage,
        site: Site,
        group: Group,
    ) -> Result<Option<Option<u64>>, String> {
        let Some(t) = self.config.founder(founder).pack else {
            return match group {
                Group::Auto => Ok(None),
                _ => Err(format!(
                    "the {} lineage keeps no group record: its herds and families are kin it senses",
                    founder.name()
                )),
            };
        };
        let young = stage != BodyStage::Adult;
        match group {
            Group::Join(id) => match self.pack(id) {
                Some(p) if p.lineage == founder => Ok(Some(Some(id))),
                Some(p) => Err(format!("pack {id} is a {} pack", p.lineage.name())),
                None => Err(format!("no pack {id}")),
            },
            Group::New if young => Err("a pack is founded by an adult; a juvenile joins one as a pup".into()),
            Group::New => Ok(Some(None)),
            Group::Auto => {
                let v = view.config.voxel_m;
                let ring = f64::from(view.config.width) * v;
                let (sx, sz) = crate::pack_centre(site, v);
                let mut best: Option<(f64, u64)> = None;
                for p in self.packs.iter().filter(|p| p.lineage == founder) {
                    let (dx, dz) = {
                        let (px, pz) = crate::pack_centre(p.den, v);
                        crate::pose::wrapped_delta(sx, sz, px, pz, ring)
                    };
                    let d = dx.hypot(dz);
                    if best.is_none_or(|(b, bid)| d < b || (d == b && p.id < bid)) {
                        best = Some((d, p.id));
                    }
                }
                match best {
                    Some((d, id)) if d <= t.territory_radius_m || young => Ok(Some(Some(id))),
                    _ if young => Err(format!("no {} pack anywhere for a pup to join", founder.name())),
                    _ => Ok(Some(None)),
                }
            }
        }
    }

    /// **The host a rider placed near `(x, z)` would hold** (the hatching rule, module note).
    fn host_near(&self, lineage: Founder, (x, z): (f64, f64), ring: f64) -> Option<u64> {
        let radius = self.config.parasite_attach_radius_m;
        let most = self.config.parasite_max_per_host as usize;
        let mut best: Option<(f64, u64)> = None;
        for a in &self.animals {
            if a.founder == Some(lineage) || a.attached.is_some() {
                continue;
            }
            let (dx, dz) = crate::pose::wrapped_delta(x, z, a.pose.x, a.pose.z, ring);
            let d = dx.hypot(dz);
            if d > radius || best.is_some_and(|(b, _)| d >= b) {
                continue;
            }
            if self.animals.iter().filter(|r| r.attached == Some(Attachment { host: a.id })).count() >= most {
                continue;
            }
            best = Some((d, a.id));
        }
        best.map(|(_, id)| id)
    }

    /// **Place an animal** of `founder` at `spot` as `spec` says (module note): its id and
    /// pack, or the eggs laid; `Err` with the reason, nothing changed. `flora` is read for an
    /// egg's site (litter, film, fungus). The lineage's controller is installed when a
    /// factory is registered.
    pub fn place_body(&mut self, world: &World, flora: &Flora, founder: Founder, spot: Spot, spec: BodySpec) -> Result<Placed, String> {
        let view = world.view();
        let phys = *self.config.founder(founder);
        if let Some(f) = spec.fullness
            && !(f.is_finite() && (0.0..=1.0).contains(&f))
        {
            return Err(format!("fullness {f} is not within 0..=1"));
        }
        if !spec.heading_rad.is_finite() {
            return Err("the heading is not finite".into());
        }
        if spec.stage == BodyStage::Egg {
            let Spot::Face(site) = spot else {
                return Err("an egg goes on a face".into());
            };
            return self.place_egg(world, flora, founder, site).map(|eggs| Placed::Eggs { site, eggs });
        }
        let rule = phys.core.reproduction;
        if spec.stage == BodyStage::Rider && !(rule.mode == BirthMode::Eggs && rule.dormant) {
            return Err(format!("the {} lineage has no riding stage", founder.name()));
        }
        let body = self.stage_body(founder, spec.stage);
        // A rider rides its host: the spot only says where to look for one.
        let stance = match (spec.stage, spot) {
            (BodyStage::Rider, Spot::Face(site)) => {
                let c = view.config;
                if site.x >= c.width || site.z >= c.depth || site.y + 1 >= c.height {
                    return Err("outside the world".into());
                }
                Stance { site, pose: Pose::at_site(site, c.voxel_m), mode: MobilityMode::Ground, perch: None }
            }
            (BodyStage::Rider, Spot::Aloft(_)) => return Err("a rider is placed near its host's face".into()),
            _ => self.stance(&view, founder, body, spot, spec.heading_rad)?,
        };
        let v = view.config.voxel_m;
        let ring = f64::from(view.config.width) * v;
        let host = if spec.stage == BodyStage::Rider {
            Some(self.host_near(founder, (stance.pose.x, stance.pose.z), ring).ok_or_else(|| {
                format!(
                    "no host within {} m for a rider (a body of another lineage, not riding, with room for one)",
                    self.config.parasite_attach_radius_m
                )
            })?)
        } else {
            None
        };
        let pack = self.resolve_group(&view, founder, spec.stage, stance.site, spec.group)?;
        let full = spec.fullness;
        let reserve = full.unwrap_or(self.config.founding.arrival_reserve).clamp(0.0, 1.0) * phys.core.reserve_of(body);
        if !self.introduce_body(world, stance.site, Species::Frondgrazer, Some(founder), body, Some(reserve), spec.heading_rad) {
            return Err("the layer refused the body".into());
        }
        let id = self.ledger.births - 1;
        self.set_arrival_stomach(id, full.unwrap_or(self.config.founding.arrival_stomach));
        let host_pose = host.and_then(|h| self.index_of(h)).map(|i| (self.animals[i].site, self.animals[i].pose));
        let i = self.index_of(id).expect("just placed");
        let a = &mut self.animals[i];
        a.pose = stance.pose;
        a.mobility.mode = stance.mode;
        a.mobility.perch = stance.perch;
        if let (Some(h), Some((site, pose))) = (host, host_pose) {
            a.attached = Some(Attachment { host: h });
            a.site = site;
            a.pose = pose;
            self.ledger.riders_attached += 1;
        }
        let pack_id = match pack {
            None => None,
            Some(Some(p)) => Some(p),
            Some(None) => {
                let p = self.next_pack;
                self.next_pack += 1;
                self.packs.push(new_pack(p, founder, stance.site, id));
                Some(p)
            }
        };
        if let Some(p) = pack_id {
            let i = self.index_of(id).expect("just placed");
            self.animals[i].pack = Some(PackMember { pack: p, pup: spec.stage != BodyStage::Adult, calling_ticks: 0 });
            crate::pack::succession(self);
        }
        self.install_founder_controller(id, founder);
        Ok(Placed::Body { id, pack: pack_id })
    }

    /// One egg of `lineage` on `site` (module note): the count laid.
    fn place_egg(&mut self, world: &World, flora: &Flora, lineage: Founder, site: Site) -> Result<u32, String> {
        let view = world.view();
        let c = view.config;
        if site.x >= c.width || site.z >= c.depth || site.y + 1 >= c.height || !view.is_support(i64::from(site.x), site.y, site.z) {
            return Err("not a support face".into());
        }
        let sc = self.config.founder(lineage).core;
        let rule = sc.reproduction;
        if rule.mode != BirthMode::Eggs || !(rule.egg_organic > 0.0) {
            return Err(format!("the {} lineage bears live young", lineage.name()));
        }
        if rule.dormant {
            return if self.bury_clutch(world, site, lineage, 1) { Ok(1) } else { Err("the bank refused the egg".into()) };
        }
        let ground = flora.view().ground_at(site);
        let fits = match self.config.founder(lineage).egg_site {
            crate::EggSite::Litter => ground.is_some_and(|g| g.litter > rule.litter_min),
            crate::EggSite::Film => {
                ground.is_some_and(|g| g.film > 0.0) && view.standing_depth_m(i64::from(site.x), site.y, site.z) > 0.0
            }
            crate::EggSite::Fungus => crate::step::fungus_near(&view, flora, site, rule.egg_fungus_radius_m),
            crate::EggSite::Shore => {
                self.config.founder(lineage).amphibious.is_some_and(|am| crate::shore_egg_site(&view, &am, site))
            }
            crate::EggSite::Vine => return Err(format!("a {} nests in the vines from a clinging parent", lineage.name())),
        };
        if !fits {
            return Err(format!("not where a {} lays ({:?})", lineage.name(), self.config.founder(lineage).egg_site));
        }
        let organic = rule.egg_organic;
        let (mineral, energy) = (sc.n_tissue * organic, sc.energy_density * organic);
        self.clutches.push(Clutch {
            site,
            count: 1,
            organic,
            mineral,
            energy,
            laid_tick: self.tick,
            lineage,
            dormancy: None,
            wet_ticks: 0,
            dry_ticks: 0,
            face: None,
        });
        self.ledger.eggs_buried += 1;
        self.ledger.introduced_organic_in += organic;
        self.ledger.introduced_mineral_in += mineral;
        self.ledger.introduced_energy_in += energy;
        Ok(1)
    }

    /// **Remove body `id`**: it leaves the world with everything it holds, booked
    /// `removed_*_out`, no carcass. A host's riders are freed where it stood; its pack
    /// runs its succession.
    pub fn remove_body(&mut self, id: u64) -> Result<(), String> {
        let i = self.index_of(id).ok_or_else(|| format!("no animal {id}"))?;
        let a = self.animals.remove(i);
        self.controllers.take(id);
        self.book_removed(&a);
        self.free_riders(&a, None);
        self.forget_body(id);
        self.percepts.remove(&id);
        self.place_memory.remove(&id);
        if a.pack.is_some() {
            crate::pack::succession(self);
        }
        Ok(())
    }

    /// Every clutch of eggs on `site` out of the world, booked `removed_*_out`: the eggs taken.
    pub fn remove_clutches(&mut self, site: Site) -> u32 {
        let mut eggs = 0;
        let mut out = (0.0, 0.0, 0.0);
        self.clutches.retain(|c| {
            if c.site != site {
                return true;
            }
            eggs += c.count;
            out.0 += c.organic;
            out.1 += c.mineral;
            out.2 += c.energy;
            false
        });
        self.ledger.removed_organic_out += out.0;
        self.ledger.removed_mineral_out += out.1;
        self.ledger.removed_energy_out += out.2;
        eggs
    }

    /// Drop body `id`'s session records (walk targets, trapped clocks, patch records…): they
    /// hold places it no longer stands at.
    fn forget_body(&mut self, id: u64) {
        self.steer.remove(&id);
        self.walk_targets.remove(&id);
        self.walk_pending.remove(&id);
        self.pressed.remove(&id);
        self.walk_shun.remove(&id);
        self.route_flee.remove(&id);
        self.forage.remove(&id);
        self.rove.remove(&id);
        self.trapped.remove(&id);
        self.drop_plans.remove(&id);
        self.hunt_paths.remove(&id);
        self.place_cache.remove(&id);
    }

    /// **Lift body `id` off the world** (module note): it stops stepping, holding its whole
    /// state; a host carries its riders, a rider lets go of its host.
    pub fn lift_body(&mut self, id: u64) -> Result<Lifted, String> {
        let i = self.index_of(id).ok_or_else(|| format!("no animal {id}"))?;
        let mut animal = self.animals.remove(i);
        if animal.attached.take().is_some() {
            self.ledger.riders_freed += 1;
        }
        let mut riders = Vec::new();
        self.animals.retain(|a| {
            if a.attached == Some(Attachment { host: id }) {
                riders.push(*a);
                false
            } else {
                true
            }
        });
        let mut lifted = Lifted { animal, riders, controllers: Vec::new(), percepts: Vec::new(), memory: Vec::new(), pack: None };
        for a in std::iter::once(lifted.animal).chain(lifted.riders.iter().copied()) {
            if let Some(c) = self.controllers.take(a.id) {
                lifted.controllers.push((a.id, c));
            }
            if let Some(p) = self.percepts.remove(&a.id) {
                lifted.percepts.push((a.id, p));
            }
            if let Some(m) = self.place_memory.remove(&a.id) {
                lifted.memory.push((a.id, m));
            }
            self.forget_body(a.id);
            self.ledger.removed_organic_out += a.stored_organic();
            self.ledger.removed_mineral_out += a.stored_mineral();
            self.ledger.removed_energy_out += a.stored_energy();
        }
        if let Some(m) = animal.pack {
            let record = self.pack(m.pack).copied();
            crate::pack::succession(self);
            if self.pack(m.pack).is_none() {
                lifted.pack = record;
            }
        }
        Ok(lifted)
    }

    /// **Set a lifted body down at `spot`**: its id. Refused (the body handed back with the
    /// reason, nothing changed) where its lineage could not be at that size.
    pub fn set_down_body(&mut self, world: &World, mut lifted: Lifted, spot: Spot) -> Result<u64, (Lifted, String)> {
        let Some(founder) = lifted.animal.founder else {
            return Err((lifted, "not a founder body".into()));
        };
        let view = world.view();
        let stance = match self.stance(&view, founder, lifted.animal.body, spot, lifted.animal.pose.heading_rad) {
            Ok(s) => s,
            Err(why) => return Err((lifted, why)),
        };
        place_at(&mut lifted.animal, &stance);
        let (site, pose) = (lifted.animal.site, lifted.animal.pose);
        for r in &mut lifted.riders {
            r.site = site;
            r.pose = pose;
        }
        Ok(self.restore(lifted))
    }

    /// **A lifted body back where it was lifted**, unchecked: the body, its riders and
    /// records reinserted and booked `introduced_*_in`. Its id.
    pub fn restore(&mut self, lifted: Lifted) -> u64 {
        let Lifted { animal, riders, controllers, percepts, memory, pack } = lifted;
        self.restore_parts(animal, &riders, &percepts, &memory, pack);
        for (id, c) in controllers {
            self.controllers.set(id, c);
        }
        animal.id
    }

    /// **A copy of a lifted body put back at its lift spot** (a save while it is held): the
    /// layer gets the body as it was, with no controller. Meant for a clone being saved.
    pub fn restore_copy(&mut self, lifted: &Lifted) {
        self.restore_parts(lifted.animal, &lifted.riders, &lifted.percepts, &lifted.memory, lifted.pack);
    }

    fn restore_parts(
        &mut self,
        animal: Animal,
        riders: &[Animal],
        percepts: &[(u64, Percept)],
        memory: &[(u64, crate::step::places::Memory)],
        pack: Option<PackRecord>,
    ) {
        if let Some(p) = pack
            && self.pack(p.id).is_none()
        {
            let at = self.packs.partition_point(|q| q.id < p.id);
            self.packs.insert(at, p);
        }
        for a in std::iter::once(animal).chain(riders.iter().copied()) {
            self.ledger.introduced_organic_in += a.stored_organic();
            self.ledger.introduced_mineral_in += a.stored_mineral();
            self.ledger.introduced_energy_in += a.stored_energy();
            self.insert(a);
        }
        for (id, p) in percepts {
            self.percepts.insert(*id, *p);
        }
        for (id, m) in memory {
            self.place_memory.insert(*id, *m);
        }
        if animal.pack.is_some() {
            crate::pack::succession(self);
        }
    }

    /// **Move body `id` to `spot`** (a lift and a set-down in one): its riders go with it,
    /// its state and group stay. Books nothing. A rider moved lets go of its host.
    pub fn move_body(&mut self, world: &World, id: u64, spot: Spot) -> Result<(), String> {
        let i = self.index_of(id).ok_or_else(|| format!("no animal {id}"))?;
        let a = self.animals[i];
        let founder = a.founder.ok_or("not a founder body")?;
        let stance = self.stance(&world.view(), founder, a.body, spot, a.pose.heading_rad)?;
        let a = &mut self.animals[i];
        if a.attached.take().is_some() {
            self.ledger.riders_freed += 1;
        }
        place_at(a, &stance);
        let (site, pose) = (a.site, a.pose);
        for r in self.animals.iter_mut().filter(|r| r.attached == Some(Attachment { host: id })) {
            r.site = site;
            r.pose = pose;
        }
        self.forget_body(id);
        Ok(())
    }
}

/// How far beside its host a freed rider looks for a face to stand on, metres from the
/// host's centre: a placement reach for the drop-off's picture, not an ecological rate.
pub(crate) const DROP_OFF_REACH_M: f64 = 1.0;

impl Fauna {
    /// **A freed rider steps off beside its host** (Wrysk, 2026-10-09): the nearest face
    /// beside the host where the rider could stand by the edit API's own rule
    /// ([`Fauna::check_spot`]), keeping the host's heading, so the drop-off happens in view
    /// rather than inside the host's body. Rings out from the host's edge to
    /// [`DROP_OFF_REACH_M`], one voxel at a time; at each distance the host's left and right
    /// first, then behind, then in front. A host off the ground (climbing, flying), or no
    /// standable face in reach, leaves the rider on the host's place, as
    /// [`crate::release`] put it. Returns whether it moved.
    pub(crate) fn step_off_beside(&mut self, view: &VoxelView<'_>, r: usize, host: &Animal) -> bool {
        let rider = self.animals[r];
        let Some((site, pose)) = self.beside_host(view, &rider, host) else {
            return false;
        };
        place_at(&mut self.animals[r], &Stance { site, pose, mode: MobilityMode::Ground, perch: None });
        true
    }

    /// **Where `rider` would step off beside `host`** ([`Fauna::step_off_beside`]'s rule, a
    /// read only): the face and pose, or `None` to stay on the host's place.
    pub fn beside_host(&self, view: &VoxelView<'_>, rider: &Animal, host: &Animal) -> Option<(Site, Pose)> {
        let rider_body = rider.body;
        let (Some(rf), Some(hf)) = (rider.founder, host.founder) else {
            return None;
        };
        if host.mobility.mode != MobilityMode::Ground || host.mobility.climb.is_some() {
            return None;
        }
        let c = view.config;
        let v = c.voxel_m;
        let rider_phys = self.config.founder(rf);
        let step = rider_phys.step_limits(v);
        let start = self.config.founder(hf).body_at(host.body).footprint_radius()
            + rider_phys.body_at(rider_body).footprint_radius();
        let (fx, fz) = host.pose.forward();
        // Left, right, behind, in front of the heading, as (dx, dz) unit vectors.
        let sides = [(fz, -fx), (-fz, fx), (-fx, -fz), (fx, fz)];
        let ring = f64::from(c.width) * v;
        let mut d = start.max(v);
        while d <= DROP_OFF_REACH_M.max(start) + 1e-9 {
            for (sx, sz) in sides {
                let x = (host.pose.x + sx * d).rem_euclid(ring);
                let z = host.pose.z + sz * d;
                if !(z >= 0.0) {
                    continue;
                }
                let (cx, cz) = ((x / v).floor() as i64, (z / v).floor() as u32);
                if cz >= c.depth {
                    continue;
                }
                let Ok(cx) = u32::try_from(cx.rem_euclid(i64::from(c.width))) else {
                    continue;
                };
                // The face in that column nearest the host's own layer, within a step.
                let mut faces = view.supports_in_column(i64::from(cx), cz);
                faces.sort_by_key(|&y| y.abs_diff(host.site.y));
                for y in faces {
                    let within = if y >= host.site.y { y - host.site.y <= step.up } else { host.site.y - y <= step.down };
                    if !within {
                        continue;
                    }
                    let site = Site { x: cx, y, z: cz };
                    if site == host.site {
                        continue;
                    }
                    if let Ok(s) = self.stance(view, rf, rider_body, Spot::Face(site), host.pose.heading_rad)
                        && s.mode == MobilityMode::Ground
                    {
                        return Some((s.site, s.pose));
                    }
                }
            }
            d += v;
        }
        None
    }
}

/// A body put on a stance: its face, pose and mode, off any wall, not falling.
fn place_at(a: &mut Animal, s: &Stance) {
    a.site = s.site;
    a.pose = s.pose;
    a.mobility = crate::Mobility { mode: s.mode, perch: s.perch, ..crate::Mobility::default() };
    a.speed_m_s = 0.0;
}

/// A pack founded by an edit on `den`, with `breeder` its first member.
fn new_pack(id: u64, lineage: Founder, den: Site, breeder: u64) -> PackRecord {
    PackRecord {
        id,
        lineage,
        den,
        breeder: Some(breeder),
        den_choice: None,
        last_litter_tick: None,
        target: None,
        chase_since_tick: 0,
        chase_progress_tick: 0,
        chase_best_m: f64::INFINITY,
        chase_work: 0.0,
        target_site: None,
        feast: None,
        feast_eaten: 0.0,
        abandoned: None,
        shunned: [(0, 0); 4],
        drawn: None,
        drawn_scent: false,
        season: Season::default(),
    }
}
