//! **The field guide's facts** (`design/handoffs/field-guide-live-2026-10-09.md`): what the
//! running world's config says about each species, for the app's species pages. Read only
//! from the config the world runs on ([`FaunaConfig`], [`FloraConfig`]), never from a table
//! of its own, so a preset or a console edit that changes a diet or a crown changes the
//! page. Served by `Session::query("guide")`.
//!
//! Per animal (every lineage in [`Founder::ALL`], by [`Founder::name`]):
//! - `length_m`, `height_m`: the adult body ([`FounderPhysiology`]'s `adult_length_m`,
//!   `adult_height_m`);
//! - `diet`: its food classes ([`FounderPhysiology::eating_diet`], [`Food::ALL`] order), each
//!   with `sources`, the plant species the class comes off (empty for the ground pools:
//!   litter, carrion, film);
//! - `prey`: the lineages on its active hunter table ([`FounderPhysiology::active_hunter`],
//!   an opportunist's narrowed to bellwings, as `is_prey_of` does), each with `when`:
//!   `grounded` for a glider (prey only on the ground), `at_rest` for a flyer (perched, on
//!   the ground or hovering inside the jaw band), else null. The size window is per body,
//!   not per lineage, so it is not here.
//!
//! Per plant (every flora [`Species::ALL`] by [`Species::name`], then the latticevine):
//! - `height_m`: a mature stand's crown top over its support face
//!   ([`cubarium_voxel_flora::SpeciesConfig::crown_height_m_at`] at `wood_max`); null for
//!   the latticevine, a cover with no height of its own;
//! - `eaten_by`: the lineages whose diet takes its tissue or its products, the exact
//!   inverse of the animals' `sources`.
//!
//! [`FounderPhysiology`]: cubarium_voxel_fauna::FounderPhysiology
//! [`FounderPhysiology::eating_diet`]: cubarium_voxel_fauna::FounderPhysiology::eating_diet
//! [`FounderPhysiology::active_hunter`]: cubarium_voxel_fauna::FounderPhysiology::active_hunter

use cubarium_voxel_fauna::{FaunaConfig, Food, Founder};
use cubarium_voxel_flora::{FloraConfig, Species};
use serde_json::{Value, json};

/// The latticevine's name in the guide (it is not a flora [`Species`]: it lives in the
/// vine cover layer).
pub const LATTICEVINE: &str = "latticevine";

/// The plants a food class comes off, by name: a stand's tissue by its income rule
/// ([`Food::of_stand`]), nectar and fruit from the species (and the vine) that make them.
pub fn sources(flora: &FloraConfig, food: Food) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Species::ALL
        .into_iter()
        .filter(|&s| {
            let c = flora.species(s);
            match food {
                Food::Foliage | Food::CapTissue => Food::of_stand(c.trophic) == food,
                Food::Nectar => c.nectar_per_s > 0.0 && c.nectar_cap > 0.0,
                Food::Fruit => c.fruit_per_s > 0.0 && c.fruit_cap > 0.0,
                Food::Litter | Food::Carrion | Food::Film => false,
            }
        })
        .map(Species::name)
        .collect();
    let vine = &flora.latticevine;
    let from_vine = match food {
        Food::Nectar => vine.nectar_per_face_s > 0.0,
        Food::Fruit => vine.fruit_per_face_s > 0.0,
        _ => false,
    };
    if from_vine {
        out.push(LATTICEVINE);
    }
    out
}

/// The lineages `hunter` strikes, with the state a body of each must be in to be taken.
pub fn prey(fauna: &FaunaConfig, hunter: Founder) -> Vec<(Founder, Option<&'static str>)> {
    let p = fauna.founder(hunter);
    let Some(h) = p.active_hunter() else {
        return Vec::new();
    };
    Founder::ALL
        .into_iter()
        .filter(|&q| h.hunts(q) && (!p.opportunist || q == Founder::Bellwing))
        .map(|q| {
            let pq = fauna.founder(q);
            let when = if pq.glider.is_some() {
                Some("grounded")
            } else if pq.flight.is_some() {
                Some("at_rest")
            } else {
                None
            };
            (q, when)
        })
        .collect()
}

/// The whole guide table: `{"animals": [...], "plants": [...]}`.
pub fn guide_table(flora: &FloraConfig, fauna: &FaunaConfig) -> Value {
    let mut eaten_by: Vec<(&'static str, Vec<&'static str>)> = Species::ALL
        .into_iter()
        .map(Species::name)
        .chain([LATTICEVINE])
        .map(|n| (n, Vec::new()))
        .collect();
    let animals: Vec<Value> = Founder::ALL
        .into_iter()
        .map(|f| {
            let p = fauna.founder(f);
            let diet: Vec<Value> = p
                .eating_diet()
                .foods()
                .map(|food| {
                    let from = sources(flora, food);
                    for name in &from {
                        if let Some((_, by)) = eaten_by.iter_mut().find(|(n, _)| n == name)
                            && !by.contains(&f.name())
                        {
                            by.push(f.name());
                        }
                    }
                    json!({ "food": food.name(), "sources": from })
                })
                .collect();
            let prey: Vec<Value> = prey(fauna, f)
                .into_iter()
                .map(|(q, when)| json!({ "name": q.name(), "when": when }))
                .collect();
            json!({
                "name": f.name(),
                "length_m": p.adult_length_m,
                "height_m": p.adult_height_m,
                "diet": diet,
                "prey": prey,
            })
        })
        .collect();
    let plants: Vec<Value> = eaten_by
        .into_iter()
        .map(|(name, by)| {
            let height = Species::parse(name).map(|s| {
                let c = flora.species(s);
                c.crown_height_m_at(c.wood_max)
            });
            json!({
                "name": name,
                "kind": if name == LATTICEVINE { "vine" } else { "stand" },
                "height_m": height,
                "eaten_by": by,
            })
        })
        .collect();
    json!({ "animals": animals, "plants": plants })
}
