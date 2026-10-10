//! **WORLD-EDIT: plants placed, removed, lifted and set down at will** (Wrysk, 2026-10-09).
//! The plant half of the edit API; `cubarium_voxel_sim::edit` is the whole-world front.
//!
//! Edits run between ticks only, like [`Flora::apply`]; a refused edit changes nothing.
//!
//! **Validity** is the founding seeder's own rule (`habitat::suitable`): the species'
//! establishment gates on the site ([`crate::FloraView::establishment_gates`]) — a support
//! face, a free place for the species' [`FaceLayer`], water under its drown depth, standing
//! water where it needs it, light, aeration and substrate — and, for a mature plant, the
//! adult light check ([`crate::adult_light_cover`] ≥ 1). The pore-water gate is read as
//! **"soil or cave damp in the root box"** rather than "wet enough this minute": a dry spell
//! does not refuse a placement the next shower would take. A seed needs a support face with
//! soil (or cave damp) under it: the bank waits for the rest.
//!
//! **Ledger.** A placed plant, a painted cover or a planted seed is booked `seeded_*_in`; a
//! removed one `removed_*_out`, as [`Command::Clear`] books one. A lift books the stand out as
//! `removed_*_out` and a set-down back in as `seeded_*_in`; a move books nothing.

use cubarium_voxel::World;
use serde::{Deserialize, Serialize};

use crate::{FaceLayer, Flora, Ground, Site, Species, Stand, find_stand, seeds};

/// The stage a placed plant arrives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlantStage {
    /// One propagule package of the species in the site's seed bank.
    Seed,
    /// A stand at the species' `alive_min` of wood.
    Seedling,
    /// A stand at the founders' largest wood fraction of `wood_max`
    /// (`founder_wood_fractions`).
    Mature,
}

impl PlantStage {
    pub fn parse(s: &str) -> Option<PlantStage> {
        match s {
            "seed" => Some(PlantStage::Seed),
            "seedling" => Some(PlantStage::Seedling),
            "mature" | "adult" => Some(PlantStage::Mature),
            _ => None,
        }
    }
}

/// What a plant placement made.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlantPlaced {
    Stand { id: u64 },
    Seeds { site: Site, organic: f64 },
}

/// **A stand held off the world** ([`Flora::lift_stand`]): its whole state. Not saved: a
/// save while it is held saves a copy back where it was lifted ([`Flora::restore_stand_copy`]).
#[derive(Clone, Debug)]
pub struct LiftedStand {
    stand: Stand,
}

impl LiftedStand {
    pub fn stand(&self) -> &Stand {
        &self.stand
    }
    pub fn id(&self) -> u64 {
        self.stand.id
    }
}

impl Flora {
    /// **Whether a `species` plant at `stage` could be on `site`** (module note); `Err` says
    /// why not. `ignore` is a stand id whose place does not count as taken (a move).
    pub fn check_plant_site(&self, world: &World, species: Species, site: Site, stage: PlantStage) -> Result<(), String> {
        self.plant_site(world, species, site, stage, None)
    }

    fn plant_site(&self, world: &World, species: Species, site: Site, stage: PlantStage, ignore: Option<u64>) -> Result<(), String> {
        let view = world.view();
        let c = view.config;
        if site.x >= c.width || site.z >= c.depth || site.y + 1 >= c.height {
            return Err("outside the world".into());
        }
        if !view.is_support(i64::from(site.x), site.y, site.z) {
            return Err("not a support face (solid with open air over it)".into());
        }
        let gates = self.view().establishment_gates(&view, site, species);
        if !(gates.soil_voxels > 0 || gates.pore_ok) {
            return Err(format!("no soil under it for a {}", species.name()));
        }
        if stage == PlantStage::Seed {
            return Ok(());
        }
        let place = self.config.face_layer(species);
        if let Ok(i) = find_stand(&self.config, &self.stands, site, place)
            && Some(self.stands[i].id) != ignore
        {
            return Err(format!("the face already holds a {}", self.stands[i].species.name()));
        }
        let sc = self.config.species(species);
        let why = if !gates.depth_ok {
            "under water deeper than it survives"
        } else if !gates.standing_ok {
            "it needs standing water there"
        } else if !gates.aeration_ok {
            "the soil is waterlogged"
        } else if !gates.light_ok {
            "too dark to establish"
        } else if !gates.substrate_ok {
            "no substrate (dead wood or litter) for it to feed on"
        } else if stage == PlantStage::Mature && crate::adult_light_cover(sc, gates.sky_visibility) < 1.0 {
            "too dark for a grown one to keep itself"
        } else {
            return Ok(());
        };
        Err(format!("{why} ({})", species.name()))
    }

    /// The wood a stand of `species` at `stage` arrives with.
    fn stage_wood(&self, species: Species, stage: PlantStage) -> f64 {
        let sc = self.config.species(species);
        match stage {
            PlantStage::Seed | PlantStage::Seedling => sc.alive_min,
            PlantStage::Mature => {
                let f = self.config.founder_wood_fractions.iter().copied().fold(0.0, f64::max);
                (f * sc.wood_max).max(sc.alive_min)
            }
        }
    }

    /// **Place a `species` plant on `site` at `stage`** (module note): the stand's id, or the
    /// seed planted; `Err` with the reason, nothing changed.
    pub fn place_plant(&mut self, world: &World, species: Species, site: Site, stage: PlantStage) -> Result<PlantPlaced, String> {
        self.check_plant_site(world, species, site, stage)?;
        if stage == PlantStage::Seed {
            let sc = self.config.species(species).clone();
            let organic = sc.propagule_package();
            if !(organic > 0.0) {
                return Err(format!("a {} makes no seed", species.name()));
            }
            let mineral = sc.n_tissue * organic;
            let gi = self.ground_slot(site);
            seeds::add_cohort(&mut self.ground[gi], species, organic, mineral, self.tick, &sc);
            if self.ground[gi].seeds.len() == 1 {
                seeds::schedule(self, site);
            }
            self.ledger.seeded_organic_in += organic;
            self.ledger.seeded_mineral_in += mineral;
            self.ledger.seeded_energy_in += sc.energy_density * organic;
            return Ok(PlantPlaced::Seeds { site, organic });
        }
        let wood = self.stage_wood(species, stage);
        let id = self.ledger.births;
        if !self.seed_founder(site, species, wood) {
            return Err("the plant layer refused the stand".into());
        }
        self.refresh_crowns(Some(world));
        Ok(PlantPlaced::Stand { id })
    }

    /// The ground entry of `site`, provisioned (its mineral booked seeded) when missing.
    fn ground_slot(&mut self, site: Site) -> usize {
        match self.ground.binary_search_by_key(&site, |g| g.site) {
            Ok(i) => i,
            Err(i) => {
                self.ledger.seeded_mineral_in += self.config.initial_mineral;
                self.ground.insert(i, Ground::new(site, self.config.initial_mineral));
                i
            }
        }
    }

    fn stand_index(&self, id: u64) -> Result<usize, String> {
        self.stands.iter().position(|s| s.id == id).ok_or_else(|| format!("no plant {id}"))
    }

    fn book_stand(&mut self, s: &Stand, sign: f64) {
        let organic = s.material();
        let energy = self.config.species(s.species).energy_density * organic;
        if sign > 0.0 {
            self.ledger.seeded_organic_in += organic;
            self.ledger.seeded_mineral_in += s.mineral;
            self.ledger.seeded_energy_in += energy;
        } else {
            self.ledger.removed_organic_out += organic;
            self.ledger.removed_mineral_out += s.mineral;
            self.ledger.removed_energy_out += energy;
        }
    }

    /// **Remove plant `id`**: its material booked `removed_*_out`.
    pub fn remove_stand(&mut self, world: &World, id: u64) -> Result<(), String> {
        let i = self.stand_index(id)?;
        let s = self.stands.remove(i);
        self.book_stand(&s, -1.0);
        self.refresh_crowns(Some(world));
        Ok(())
    }

    /// **Lift plant `id` off the world**, its whole state kept.
    pub fn lift_stand(&mut self, world: &World, id: u64) -> Result<LiftedStand, String> {
        let i = self.stand_index(id)?;
        let stand = self.stands.remove(i);
        self.book_stand(&stand, -1.0);
        self.refresh_crowns(Some(world));
        Ok(LiftedStand { stand })
    }

    /// **Set a lifted plant down on `site`** (a transplant: its state kept, its place the
    /// species' own). Refused, the plant handed back, where it could not grow.
    pub fn set_down_stand(&mut self, world: &World, lifted: LiftedStand, site: Site) -> Result<u64, (LiftedStand, String)> {
        let stage = self.stand_stage(&lifted.stand);
        if let Err(why) = self.check_plant_site(world, lifted.stand.species, site, stage) {
            return Err((lifted, why));
        }
        let mut stand = lifted.stand;
        self.relocate(&mut stand, site);
        let id = self.insert_stand(stand);
        self.refresh_crowns(Some(world));
        Ok(id)
    }

    /// The stage a standing plant counts as for the site rule: mature from its first grown
    /// profile.
    fn stand_stage(&self, s: &Stand) -> PlantStage {
        if s.profile_stage >= 1 { PlantStage::Mature } else { PlantStage::Seedling }
    }

    fn relocate(&mut self, stand: &mut Stand, site: Site) {
        stand.site = site;
        stand.place = self.config.face_layer(stand.species);
        stand.support = None;
        self.ground_slot(site);
    }

    fn insert_stand(&mut self, stand: Stand) -> u64 {
        let id = stand.id;
        self.book_stand(&stand, 1.0);
        let at = match find_stand(&self.config, &self.stands, stand.site, stand.place) {
            Ok(i) | Err(i) => i,
        };
        self.stands.insert(at, stand);
        id
    }

    /// **A lifted plant back where it was lifted**, unchecked. Its id.
    pub fn restore_stand(&mut self, world: &World, lifted: LiftedStand) -> u64 {
        let id = self.insert_stand(lifted.stand);
        self.refresh_crowns(Some(world));
        id
    }

    /// **A copy of a lifted plant put back where it was lifted** (a save while it is held),
    /// meant for a clone being saved.
    pub fn restore_stand_copy(&mut self, lifted: &LiftedStand) {
        self.insert_stand(lifted.stand);
        self.refresh_crowns(None);
    }

    /// **Move plant `id` to `site`**: a transplant in place, its state kept; books nothing.
    pub fn move_stand(&mut self, world: &World, id: u64, site: Site) -> Result<(), String> {
        let i = self.stand_index(id)?;
        let stage = self.stand_stage(&self.stands[i]);
        self.plant_site(world, self.stands[i].species, site, stage, Some(id))?;
        let mut stand = self.stands.remove(i);
        self.relocate(&mut stand, site);
        let at = match find_stand(&self.config, &self.stands, stand.site, stand.place) {
            Ok(i) | Err(i) => i,
        };
        self.stands.insert(at, stand);
        self.refresh_crowns(Some(world));
        Ok(())
    }

    /// **Paint `species` over `sites`** (cover such as springturf, or any stand species):
    /// on each face that passes the site rule for a mature plant and holds none, a stand of
    /// `amount` (`0..=1`) of the species' `wood_max`. The faces painted and, for the rest,
    /// why not.
    pub fn paint(&mut self, world: &World, species: Species, sites: &[Site], amount: f64) -> (Vec<u64>, Vec<(Site, String)>) {
        let sc = self.config.species(species);
        let wood = amount * sc.wood_max;
        let (mut done, mut refused) = (Vec::new(), Vec::new());
        if !(wood.is_finite() && wood >= sc.alive_min) {
            refused.extend(sites.iter().map(|s| (*s, format!("amount {amount} is below a living {}", species.name()))));
            return (done, refused);
        }
        for &site in sites {
            if let Err(why) = self.check_plant_site(world, species, site, PlantStage::Mature) {
                refused.push((site, why));
                continue;
            }
            let id = self.ledger.births;
            if self.seed_founder(site, species, wood) {
                done.push(id);
            } else {
                refused.push((site, "the plant layer refused the stand".into()));
            }
        }
        self.refresh_crowns(Some(world));
        (done, refused)
    }

    /// **Clear `sites`**: every stand on each, and every latticevine rooted there, booked
    /// `removed_*_out`. `(stands, vines)` removed.
    pub fn clear_sites(&mut self, world: &World, sites: &[Site]) -> (usize, usize) {
        let (mut stands, mut vines) = (0, 0);
        for &site in sites {
            let run = crate::on_site(&self.stands, site);
            for s in self.stands.drain(run).collect::<Vec<_>>() {
                self.book_stand(&s, -1.0);
                stands += 1;
            }
            vines += self.clear_vines_at(site);
        }
        self.refresh_crowns(Some(world));
        (stands, vines)
    }

    /// The stands on `site`, for a picker.
    pub fn stands_at(&self, site: Site) -> &[Stand] {
        &self.stands[crate::on_site(&self.stands, site)]
    }
}

impl FaceLayer {
    /// A short name for a picker.
    pub fn name(self) -> &'static str {
        match self {
            FaceLayer::Plant => "plant",
            FaceLayer::Under => "under",
            FaceLayer::Fungus => "fungus",
        }
    }
}
