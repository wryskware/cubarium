//! The **shared harness**: the experiment conditions and placement helpers both voxel
//! flora studies run on, in one place so that they cannot drift apart.
//!
//! Every item here is an **experiment condition and not a model rule**. The model has no
//! opinion about where a founder is planted, how much rain falls or how long a warm-up
//! lasts: `Command::Seed` is accepted on any support face whatever the gates say, rain is a
//! `cubarium_voxel::Config` field, and the warm-up is this harness counting ticks. What the
//! model does own — the establishment predicate, the crown geometry, the gate values — is
//! read here through the model's own API and never re-implemented ([`passes`],
//! [`gate_line`], [`canopy_over`]).
//!
//! It is a module file included by `#[path]` from each example rather than a `pub` module of
//! the crate, because a study's placement rules have no business in the library a preset
//! author reads: the crate would then export "bloomcrown belongs on a ridge", which is this
//! harness's statement about an experiment and not the species' contract.
//!
//! Astra's round 8 named the pieces of it that had to be reusable: [`canopy_over`] and
//! [`covered_by`] on **absolute** tops, the `OpenSoil` keyed draw, no off-predicate
//! fallback, and a founder's gate values reported by identity. `two_producers` and
//! `replacement` now share exactly those, with one copy each.

// Each example uses a different part of this module, so the other part is dead code in that
// target. That is the price of one copy of the helpers, and it is cheaper than two.
#![allow(dead_code)]

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, World};
use cubarium_voxel_flora::{Flora, Site, SkyCache, Species, SpeciesConfig};

pub const WARMUP_TICKS: u32 = 1000;
pub const FOUNDERS_PER_SPECIES: usize = 8;
/// A founder starts at this fraction of its own species' `wood_max`.
///
/// It was a flat 0.3 while both species had `wood_max` 0.6, and 0.3 is what half of 0.6
/// still is, so the two original arms are unchanged to the bit. It has to be a fraction now
/// because the round-4 presets are smaller bodies — springturf's whole `wood_max` is 0.06 —
/// and `Command::Seed` does not bound its `wood` by `wood_max`: a flat 0.3 would have
/// planted a springturf five times its own maximum size, which can never grow and is not a
/// founder of anything. At half of `wood_max` a founder also starts at exactly `donor_min`
/// for all five presets, which is where a stand becomes able to reproduce.
pub const FOUNDER_FRACTION: f64 = 0.5;

pub fn founder_wood(sc: &SpeciesConfig) -> f64 {
    FOUNDER_FRACTION * sc.wood_max
}

/// Where on its own eligible skyline a species' founders go, and why.
///
/// This is the harness's statement of the **contract habitat** of
/// `design/handoffs/voxel-round4-presets-briefs-2026-09-17.md`, and it is an **experiment
/// condition and not a model rule**: the model has no opinion about where a founder is
/// planted, because `Command::Seed` is accepted on any support face whatever the gates say.
/// Every run prints the rule it used beside the count it planted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Habitat {
    /// The **highest** eligible faces: an open ridge. Bloomcrown.
    Ridge,
    /// The **lowest** eligible faces: a wet hollow. Umbrellafrond.
    Hollow,
    /// **Bare open soil**: gate-passing faces whose own support voxel is soil, that
    /// nothing is standing on and that have no crown over them, drawn in a deterministic
    /// keyed order. Springturf.
    ///
    /// It was "the wettest root box, wettest first" until Astra's R7.4. Two things were
    /// wrong with that. It never tested the support material at all, so "bare soil" was
    /// not a condition of it; and ordering a gate-passing pool by mean pore is a habitat
    /// claim the role does not make — "pioneer of open **moist** soil" means not dry, which
    /// is what the pore gate already says, and the wettest ground available is a different
    /// and stronger statement. The keyed draw takes a spread of the qualifying pool instead
    /// and orders it by nothing the role claims.
    OpenSoil,
    /// Eligible faces whose own support voxel is **not** soil. Since the pore gate has
    /// already passed on them, a soil pocket is by definition in reach: that is what
    /// `pore_ok` on a rock face means. Stonecushion.
    RockWithAPocket,
    /// Eligible faces under a crown whose **absolute** top — support height plus crown
    /// height — is above the one this species' own founder would have there, which means
    /// this species is planted after the others. Velvetpad.
    UnderACrown,
    /// Faces whose **mycelium box holds a declared log**. Glowcap.
    ///
    /// The gate does the whole of the filtering here, and it is the only habitat rule of
    /// the six for which that is true: a saprotroph's establishment predicate reads the
    /// dead wood of its own box, so the eligible pool this rule is handed already *is* the
    /// set of faces with something to eat, and all the rule adds is the keyed spread
    /// `OpenSoil` uses. A fresh world holds no dead wood at all, so `community` lays the
    /// logs down first with `DepositKind::DeadWood` and prints what it declared.
    OnALog,
}

/// The table itself. One line per species, and the only place the harness says where a
/// species belongs.
pub fn habitat_of(species: Species) -> Habitat {
    match species {
        Species::Bloomcrown => Habitat::Ridge,
        Species::Umbrellafrond => Habitat::Hollow,
        Species::Springturf => Habitat::OpenSoil,
        Species::Stonecushion => Habitat::RockWithAPocket,
        Species::Velvetpad => Habitat::UnderACrown,
        Species::Glowcap => Habitat::OnALog,
        // Package N: the canopy on the high open ground, the shrub on open soil, the reed
        // in the wet hollow. Harness placement only; the gates still decide.
        Species::Vaulttree => Habitat::Ridge,
        Species::Lanternberry => Habitat::OpenSoil,
        Species::Siphonreed => Habitat::Hollow,
    }
}

/// The species this species has to be planted after: `UnderACrown` needs crowns to be
/// under, so it goes last.
pub fn planted_last(species: Species) -> bool {
    habitat_of(species) == Habitat::UnderACrown
}

/// The harness's rain, metres per second onto exposed top surfaces: an experiment
/// condition, not a model knob and not a tuned plant parameter.
///
/// 2e-4 m/s over the default 192 m² footprint is a nominal 0.0384 m³/s, which is 77 % of
/// the outlet's 0.05 m³/s. Actual accepted rain is at most that, because only exposed top
/// faces take it, and the outlet can export less than its capacity when its own cell is
/// undersupplied — so this bounds the *input* below the exit and leaves whether the head
/// settles to the printed budget. The previous 5e-4 was 0.096 m³/s, nearly twice what the
/// world can remove (Astra R4.2).
pub const HARNESS_RAIN_M_PER_S: f64 = 0.0002;

/// The core's water ledger and stores at one instant, so an interval's budget is a
/// difference of two of these and nothing has to be accumulated by hand.
#[derive(Clone, Copy, Debug)]
pub struct WaterMark {
    pub seconds: f64,
    pub rain_in: f64,
    pub outlet_out: f64,
    pub evaporation_out: f64,
    pub transpiration_out: f64,
    pub stored: f64,
    pub head_m: f64,
}

pub fn water_mark(world: &World, seconds: f64) -> WaterMark {
    let v = world.view();
    WaterMark {
        seconds,
        rain_in: v.ledger.rain_in,
        outlet_out: v.ledger.outlet_out,
        evaporation_out: v.ledger.evaporation_out,
        transpiration_out: v.ledger.transpiration_out,
        stored: v.stored_m3(),
        head_m: world.aquifer_head_m(),
    }
}

/// One interval of the water budget, as rates, plus the head at its end: in, out, and the
/// storage change they explain. `in - out - storage` is the core's own conservation
/// residual over the interval and should be float noise.
pub fn water_budget_line(a: &WaterMark, b: &WaterMark) -> String {
    let dt = (b.seconds - a.seconds).max(1e-12);
    let rain = (b.rain_in - a.rain_in) / dt;
    let outlet = (b.outlet_out - a.outlet_out) / dt;
    let evap = (b.evaporation_out - a.evaporation_out) / dt;
    let transp = (b.transpiration_out - a.transpiration_out) / dt;
    let storage = (b.stored - a.stored) / dt;
    format!(
        "water m3/s over {dt:.0} s: rain in {rain:.6}, outlet {outlet:.6}, evaporation \
         {evap:.6}, transpiration {transp:.8}, storage change {storage:+.6} (residual \
         {:+.2e}); head {:.3} m ({:+.4} m), stored {:.4} m3",
        rain - outlet - evap - transp - storage,
        b.head_m,
        b.head_m - a.head_m,
        b.stored
    )
}

/// A column a founder was planted in. The `y` is not part of it: under another noise
/// seed the same column's support face may sit a voxel higher or lower, and it is the
/// same place on the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Founder {
    pub species: Species,
    pub x: u32,
    pub z: u32,
}

/// One [`Habitat`] rule applied to a pool of eligible sites: the candidates, best first.
/// The strided sample the caller then takes is over this order, so the rule is the whole of
/// what makes one species' founders differ from another's.
///
/// `skyline` reaches here already sorted by `(y, x, z)`, so `Hollow` is the pool as it
/// stands and `Ridge` is it reversed.
pub fn order_for(
    world: &World,
    flora: &Flora,
    species: Species,
    habitat: Habitat,
    mut pool: Vec<Site>,
    planted: &[Founder],
) -> Vec<Site> {
    let view = world.view();
    match habitat {
        Habitat::Hollow => pool,
        Habitat::Ridge => {
            pool.reverse();
            pool
        }
        Habitat::OpenSoil => {
            // Astra R7.4. Four conditions, and a draw that adds no fifth one: the support
            // face is soil, nothing already stands there, no column of `planted` has
            // reserved it, and no crown reaches over it. Gate-passing is the caller's:
            // `pool` is the eligible set and there is no longer any fallback to sites that
            // fail the predicate.
            let canopy = canopy_over(world, flora, planted);
            pool.retain(|s| {
                view.material_at(s.x as i64, s.y, s.z) == cubarium_voxel::Material::Soil
                    && flora.view().stand_at(*s).is_none()
                    && !planted.iter().any(|f| f.x == s.x && f.z == s.z)
                    && !covered_by(&canopy, view.config.width, *s, s.y as f64)
            });
            // A deterministic keyed order: the same pool always yields the same founders,
            // and the order says nothing about how wet a site is.
            pool.sort_by_key(|s| (site_key(species, *s), s.x, s.z));
            pool
        }
        Habitat::RockWithAPocket => {
            // A support face that is not soil. The pore gate has already passed on every
            // site in the pool, and the root box only ever holds soil voxels, so a
            // non-soil face in this pool *is* a face with a soil pocket in reach.
            pool.retain(|s| {
                view.material_at(s.x as i64, s.y, s.z) != cubarium_voxel::Material::Soil
            });
            pool.reverse();
            pool
        }
        Habitat::OnALog => {
            // Everything that makes a face a glowcap's face is already in `pool`: the
            // substrate gate passed on it, which is what "there is a log here" means. Drop
            // what is occupied or reserved, then take the same keyed spread `OpenSoil`
            // takes, so the founders are a sample of the declared logs and not the first
            // few in site order.
            pool.retain(|s| {
                flora.view().stand_at(*s).is_none()
                    && !planted.iter().any(|f| f.x == s.x && f.z == s.z)
            });
            pool.sort_by_key(|s| (site_key(species, *s), s.x, s.z));
            pool
        }
        Habitat::UnderACrown => {
            // One canopy for the whole pool: living stands at their actual site, wood and
            // foliage, and planned founders at their own column's support face (Astra
            // R7.1).
            let canopy = canopy_over(world, flora, planted);
            let width = view.config.width;
            pool.retain(|s| under_a_crown(&canopy, width, flora, species, *s));
            pool
        }
    }
}

/// One crown, as the model shades with it: an **absolute** top over the world floor, a
/// radius, and the column it stands over. `step.rs`'s `crown_of` builds the same thing from
/// a stand.
#[derive(Clone, Copy, Debug)]
pub struct Canopy {
    pub x: f64,
    pub z: f64,
    /// `site.y + crown_height(wood)`. The support height is part of it, which is what the
    /// harness's level-face test used to leave out (Astra R7.1).
    pub top: f64,
    pub radius: f64,
    /// The crown's own foliage. The shade *condition* does not read it — `step.rs` shades
    /// on cover and a higher top alone — but its *strength* is
    /// `exp(-shade_k · foliage / area)`, so a crown with no leaves on it attenuates by
    /// nothing and this helper does not call such a site shaded (Astra R7.1: actual
    /// foliage, not a founder's).
    pub foliage: f64,
}

/// Every crown over this world: one per living stand at its **actual** site and wood, plus
/// one per planned-but-unplanted founder resolved at its own column's support face in this
/// world and at the wood a founder is planted with.
///
/// Resolving the plan's support face is the R7.1/R7.4 repair: a `Founder` record
/// deliberately carries no `y`, but the height of the ground it will stand on is in the
/// world already, and without it a crown's top cannot be compared with anything.
pub fn canopy_over(world: &World, flora: &Flora, planned: &[Founder]) -> Vec<Canopy> {
    let view = flora.view();
    let mut out: Vec<Canopy> = Vec::with_capacity(view.stands.len() + planned.len());
    for stand in view.stands {
        let sc = flora.config().species(stand.species);
        out.push(Canopy {
            x: f64::from(stand.site.x),
            z: f64::from(stand.site.z),
            top: f64::from(stand.site.y) + sc.crown_height(stand.wood, flora.config().voxel_m),
            radius: sc.crown_radius(stand.wood, flora.config().voxel_m),
            foliage: stand.foliage,
        });
    }
    for f in planned {
        if view
            .stands
            .iter()
            .any(|s| s.site.x == f.x && s.site.z == f.z)
        {
            continue; // already standing, and counted above with its real wood
        }
        let Some(site) = cubarium_voxel_flora::highest_support(&world.view(), f.x as i64, f.z)
        else {
            continue;
        };
        let sc = flora.config().species(f.species);
        let wood = founder_wood(sc);
        out.push(Canopy {
            x: f64::from(f.x),
            z: f64::from(f.z),
            top: f64::from(site.y) + sc.crown_height(wood, flora.config().voxel_m),
            radius: sc.crown_radius(wood, flora.config().voxel_m),
            // What `Command::Seed` will give it: `alpha · wood`, full foliage.
            foliage: sc.alpha * wood,
        });
    }
    out
}

/// Whether any crown covers this column and has its top **strictly above** `top` — the
/// model's own two shade conditions (`step.rs`'s `light_per_stand`): wrapped cover in `x`,
/// plain cover in `z`, inside the crown's radius, and a strictly higher top.
///
/// `top` is an absolute height, so a caller asks "is there anything over this ground" with
/// the site's own face and "would this stand be shaded" with the stand's own crown top.
pub fn covered_by(canopy: &[Canopy], width: u32, site: Site, top: f64) -> bool {
    let width = f64::from(width.max(1));
    canopy.iter().any(|c| {
        if c.top <= top || c.foliage <= 0.0 {
            return false;
        }
        let dx = wrapped_delta(c.x, f64::from(site.x), width);
        let dz = c.z - f64::from(site.z);
        dx * dx + dz * dz <= c.radius * c.radius
    })
}

/// `a - b` on a world that wraps in `x`, the same way `step.rs`'s own `wrapped_delta` does.
pub fn wrapped_delta(a: f64, b: f64, period: f64) -> f64 {
    let mut d = a - b;
    while d > period * 0.5 {
        d -= period;
    }
    while d < -period * 0.5 {
        d += period;
    }
    d
}

/// A deterministic key for one column and species: splitmix64 over `(x, z)` salted by the
/// species index. It lets a habitat rule draw a repeatable spread of a qualifying pool
/// **without** ordering that pool by any quantity, which is what Astra's R7.4 asked for in
/// place of wettest-first. Two species draw different orders from the same pool.
pub fn site_key(species: Species, site: Site) -> u64 {
    let salt = (species.index() as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut z = ((u64::from(site.x) << 32) ^ u64::from(site.z)).wrapping_add(salt);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// One site's gate values, as the numbers the predicate itself read, each beside the
/// threshold it is compared with.
pub fn gate_line(world: &World, flora: &Flora, species: Species, site: Site) -> String {
    let sc = flora.config().species(species);
    let g = flora
        .view()
        .establishment_gates(&world.view(), site, species);
    format!(
        "mean available water {} (>= {:.2}), saturated fraction {:.3} (<= {:.2}), water {:.3} m \
         (<= {:.2}), sky {:.3} (>= {:.2}), {} soil voxels, dead wood in the box {:.3} \
         (>= {:.3})",
        g.mean_water
            .map_or_else(|| "none".to_string(), |m| format!("{m:.3}")),
        sc.establish_water_min,
        g.saturated_fraction,
        sc.establish_saturated_max,
        g.water_depth_m,
        sc.drown_depth_m,
        g.sky_visibility,
        sc.establish_light_min,
        g.soil_voxels,
        g.dead_wood,
        sc.establish_substrate_min
    )
}

/// Whether a founder planted at this site would stand under a crown that the **model**
/// would let shade it: some crown covers the column — wrapped in `x`, plain in `z`, inside
/// its own radius — and its **absolute** top is strictly above this founder's own absolute
/// top. Those are `step.rs`'s two conditions in `light_per_stand`, read off the same
/// `crown_height`/`crown_radius` the model shades with.
///
/// Before Astra's R7.1 this compared the two crown heights **over their own support faces**
/// and left both support heights out, which is not conservative — it is wrong in both
/// directions. Astra's case: a half-grown bloomcrown on a face at `y = 2` tops out at 4.0
/// with radius 1, a half-grown velvetpad on a face at `y = 4` tops out at 4.75, and the old
/// test admitted the pad because `2.0 > 0.75` — while the model correctly applies no shade
/// at all. The other direction is a short plant on high ground, which does shade a pad below
/// it and which the old test refused. It also read every existing stand as a half-grown
/// founder, so an old or a newborn resident carried the wrong crown entirely; `canopy_over`
/// now reads a standing stand's own site, wood and foliage.
pub fn under_a_crown(
    canopy: &[Canopy],
    width: u32,
    flora: &Flora,
    species: Species,
    site: Site,
) -> bool {
    let sc = flora.config().species(species);
    let own_top = f64::from(site.y) + sc.crown_height(founder_wood(sc), flora.config().voxel_m);
    covered_by(canopy, width, site, own_top)
}

/// The species' establishment predicate at one site: the model's own, through
/// `FloraView::can_establish`.
///
/// This used to be the harness's own approximation of it — the support voxel's pore
/// fraction and a binary saturation test, where the model reads the capacity-weighted mean
/// and the saturated *fraction* over the whole root box — with a line-for-line replica of
/// the model's private predicate further down the file for the germination diagnosis to
/// use. Package I's report had to carry a caveat saying which number came from which.
/// Package J exposed the model's, so there is one predicate: founder selection, the
/// habitat sets, the off-predicate count and the diagnosis all read it.
/// Round 5b: it reads `FloraView::can_establish` and not the free `can_establish`, because
/// a saprotroph's substrate gate reads the dead wood of its own mycelium box and a
/// `VoxelView` holds none. Still exactly one predicate — the flora-view form *is* the free
/// one with the substrate supplied — and it is now the same one for all six species.
pub fn passes(world: &World, flora: &Flora, species: Species, s: Site) -> bool {
    flora.view().can_establish(&world.view(), s, species)
}

/// Generate, open the outlet, warm up. The same preparation `run` does, factored out so
/// the probe cannot drift from it. The **core's own** outlet capacity, so every published
/// `two_producers` run means exactly what it meant.
pub fn prepared_world(seed: u64, noise_seed: u64) -> World {
    prepared_world_with_outlet(seed, noise_seed, VoxelConfig::default().outlet_m3_per_s)
}

/// The nominal **accepted rain** of a world of this footprint, m³/s: the harness's rain over
/// the whole plan area, `rain · width · depth · voxel_m²`. 0.0384 for the default 128 × 24
/// voxels of 0.25 m, which is 192 m².
///
/// It is nominal because only exposed top faces take rain; on the default generated world
/// every column has one, and the measured `rain_in` is exactly this.
pub fn nominal_accepted_rain() -> f64 {
    let c = VoxelConfig::default();
    HARNESS_RAIN_M_PER_S * f64::from(c.width) * f64::from(c.depth) * c.voxel_m * c.voxel_m
}

/// [`prepared_world`] with the outlet's **capacity** named by the caller: an experiment
/// condition a study declares once, rather than the core's default inherited by accident
/// (Astra R11.3).
///
/// The core's 0.05 m³/s is a **capacity and not an imposed export** — `water.rs` takes only
/// the free water actually at the outlet cell — but a capacity above the accepted rain is a
/// world that drains whenever there is water to drain, which is what the round-10 smoke
/// measured: a constant deficit of 0.0116 m³/s, 30.2 % of rain. A **stationary** study
/// declares a capacity equal to its own accepted input instead ([`nominal_accepted_rain`]),
/// leaving the rain and the soil-wetting treatment exactly as they were. Raising rain to
/// 2.60417e-4 m/s would equalise the same two numbers by changing how wet the soil gets,
/// which is a different treatment.
///
/// Neither equality guarantees equilibrium: evaporation and transpiration also leave, and the
/// conditioning tolerances still have to pass on both phases (R11.3).
pub fn prepared_world_with_outlet(seed: u64, noise_seed: u64, outlet_m3_per_s: f64) -> World {
    let dry = VoxelConfig {
        seed,
        noise_seed,
        rain_m_per_s: HARNESS_RAIN_M_PER_S,
        outlet_m3_per_s,
        ..VoxelConfig::default()
    };
    let basin_floor_m = World::new(dry.clone())
        .outlet_cell()
        .map_or(0.0, |(_, y, _)| y as f64)
        * dry.voxel_m;
    let config = VoxelConfig {
        initial_aquifer_head_m: basin_floor_m + 1.0,
        ..dry
    };
    let mut world = World::new(config);
    world.apply(WorldCommand::SetOutlet { open: true });
    for _ in 0..WARMUP_TICKS {
        world.step();
    }
    world
}

/// Every column's own highest support face, sorted low to high.
pub fn skyline_of(world: &World) -> Vec<Site> {
    let (width, depth) = (world.config().width, world.config().depth);
    let mut skyline: Vec<Site> = Vec::new();
    for z in 0..depth {
        for x in 0..width as i64 {
            if let Some(site) = cubarium_voxel_flora::highest_support(&world.view(), x, z) {
                skyline.push(site);
            }
        }
    }
    skyline.sort_by_key(|s| (s.y, s.x, s.z));
    skyline
}

/// A species' own habitat on this skyline, ordered best first by its own [`Habitat`] rule —
/// the same table `pick_founders` uses, so the probe cannot disagree with the experiment
/// about where a species belongs.
///
/// `UnderACrown` reads the crowns of whatever is **standing**, so a probe's newcomer goes
/// under the resident it is invading rather than into the open. It is passed no planned
/// founders: `canopy_over` takes living stands straight off the flora view, at their own
/// site, wood and foliage. This function used to hand them over as `Founder` records, which
/// threw their wood away and read an old or a newborn resident as a half-grown founder
/// (Astra R7.1).
pub fn habitat(world: &World, flora: &Flora, skyline: &[Site], species: Species) -> Vec<Site> {
    let ok: Vec<Site> = skyline
        .iter()
        .copied()
        .filter(|s| passes(world, flora, species, *s))
        .collect();
    order_for(world, flora, species, habitat_of(species), ok, &[])
}

/// Exactly `ticks` coupled ticks, which is what a conditioning interval counts in: the
/// seconds a phase reports are `ticks · DT` and never a requested duration (R11.2).
pub fn step_coupled_ticks(flora: &mut Flora, world: &mut World, ticks: u64) {
    for _ in 0..ticks {
        world.step();
        flora.step(world);
    }
}

pub fn step_coupled(flora: &mut Flora, world: &mut World, seconds: f64) {
    let ticks = (seconds * cubarium_voxel::TICK_HZ as f64).round() as u64;
    for _ in 0..ticks {
        world.step();
        flora.step(world);
    }
}

pub fn count(flora: &Flora, species: Species) -> usize {
    flora
        .view()
        .stands
        .iter()
        .filter(|s| s.species == species)
        .count()
}

// ============================================================ the observation cap
//
// Astra's R5.4, corrected in R7.2: a replacement control has to **predeclare** how long it
// would take one newborn to become a donor and fund one package, from the presets' own
// arithmetic, and print it before the run. Otherwise a window that ends without a
// replacement is read as exclusion when it is only short: 6,000 s cannot resolve
// bloomcrown's own 3,008.15 s pair with anything left over, and it cannot resolve
// stonecushion's 8,647.25 s at all.

/// The predeclared observation cap of **one species**: the time from a newborn of it to a
/// donor at the model's own growth cap, plus the time to save one whole package at its own
/// funding rate. Every number in it is read off [`SpeciesConfig`]; none of it is tuned and
/// none of it is measured.
///
/// **It is a lower bound and it is declared as one.** Two model rules make it so.
/// `step.rs`'s growth cap allows at most `wood_rate · W · dt` of new wood per tick, so a
/// stand's wood can at best follow `W · (1 + wood_rate · dt)` per tick and never faster —
/// but a stand only grows that fast while its income covers maintenance, both demands and
/// the reserve, and a shaded or thirsty one takes longer. And `propagate` funds a parcel out
/// of the reserve **above** `donor_reserve_floor`, so `propagule_rate` is what a donor asks
/// for and not what it gets. The second branch of the growth cap — the `wood_max − W`
/// headroom — is left out for the same reason: it can only make the model slower, never
/// faster, and it does not bind below `donor_min` for any of the six presets.
///
/// So "unresolved at cap" is the honest reading of a window that ends without a
/// replacement, and inferring exclusion from one is what R5.4 refuses.
#[derive(Clone, Copy, Debug)]
pub struct ObservationCap {
    pub species: Species,
    /// `alive_min`: the wood a germination builds, which is exactly what one package's
    /// `w_frac` share pays for.
    pub newborn_wood: f64,
    /// `donor_min`: the wood a stand needs before it may set anything aside at all.
    pub donor_wood: f64,
    /// Ticks of the geometric growth cap from `newborn_wood` to `donor_wood`, rounded up:
    /// the first tick whose wood is at or above `donor_min`.
    pub growth_ticks: u64,
    pub growth_s: f64,
    /// `alive_min / w_frac`: one whole package, the smallest thing a donor can send.
    pub package: f64,
    /// Ticks to save that package at `propagule_rate · dt` gross, of which `1 / (1 + c_g)`
    /// reaches the parcel, with the reserve never binding. Rounded up.
    pub package_ticks: u64,
    pub package_s: f64,
    /// `growth_s + package_s`: newborn to funded donor, the replacement time this cap
    /// declares.
    pub total_s: f64,
}

/// The cap of one species, from its own preset. `f64::INFINITY` where a preset cannot get
/// there at all — a zero `wood_rate`, a zero `propagule_rate`, a zero `w_frac` — because a
/// species that cannot replace itself has no finite cap and the harness must say so rather
/// than print a number.
pub fn observation_cap(species: Species, sc: &SpeciesConfig) -> ObservationCap {
    let dt = cubarium_voxel_flora::DT;
    let (newborn_wood, donor_wood) = (sc.alive_min, sc.donor_min);
    // The growth cap, per tick: `W · (1 + wood_rate · dt)`. `ceil` of the exact solution is
    // the first tick at or above `donor_min`, which is what 2,708.15 s is.
    let step = 1.0 + sc.wood_rate * dt;
    let growth_ticks = if donor_wood <= newborn_wood {
        0.0
    } else if step <= 1.0 || newborn_wood <= 0.0 {
        f64::INFINITY
    } else {
        ((donor_wood / newborn_wood).ln() / step.ln()).ceil()
    };
    // One whole package, net of construction respiration: of a gross `propagule_rate · dt`
    // taken out of the reserve, `1 / (1 + c_g)` of it reaches the parcel.
    let package = if sc.propagule_split[0] > 0.0 {
        sc.alive_min / sc.propagule_split[0]
    } else {
        0.0
    };
    let net_per_tick = sc.propagule_rate * dt / (1.0 + sc.build);
    let package_ticks = if package <= 0.0 {
        f64::INFINITY
    } else if net_per_tick <= 0.0 {
        f64::INFINITY
    } else {
        (package / net_per_tick).ceil()
    };
    let (growth_s, package_s) = (growth_ticks * dt, package_ticks * dt);
    ObservationCap {
        species,
        newborn_wood,
        donor_wood,
        growth_ticks: finite_ticks(growth_ticks),
        growth_s,
        package,
        package_ticks: finite_ticks(package_ticks),
        package_s,
        total_s: growth_s + package_s,
    }
}

fn finite_ticks(t: f64) -> u64 {
    if t.is_finite() && t >= 0.0 {
        t as u64
    } else {
        u64::MAX
    }
}

impl ObservationCap {
    /// The arithmetic, in one line, with every input in it: a reader can recompute the cap
    /// from what is printed and nothing is implied.
    pub fn line(&self, sc: &SpeciesConfig) -> String {
        format!(
            "{:>14}: newborn wood {} -> donor {} at wood_rate {} under the growth cap = \
             {} ticks = {:.2} s; one package {:.4} ({}/{}) at propagule_rate {} net of build \
             {} = {} ticks = {:.2} s; **cap {:.2} s**",
            self.species.name(),
            self.newborn_wood,
            self.donor_wood,
            sc.wood_rate,
            self.growth_ticks,
            self.growth_s,
            self.package,
            sc.alive_min,
            sc.propagule_split[0],
            sc.propagule_rate,
            sc.build,
            self.package_ticks,
            self.package_s,
            self.total_s
        )
    }
}

/// The **earliest-possible replacement timeline** of one species, measured from the moment
/// this study actually introduces it: **one founder with a zero parcel, and no newborn of
/// its own anywhere in the world**.
///
/// Astra's R10.1. [`ObservationCap`] costs *newborn → donor → package*, which is the right
/// arithmetic for a newborn — and the study has none. Its founder has to fund and deliver a
/// first package, that package has to germinate, and only then does the newborn clock the
/// cap describes begin; then that descendant has to fund a package of its own before
/// anything has been *replaced*. Four stages, in the model's own phase order:
///
/// 1. **Fund and deliver the first package.** `propagate` funds `propagule_rate · dt`
///    gross into the parcel and, **in the same tick, after that funding**, sends one whole
///    package if the parcel now holds one. There is no separate delivery phase, so this
///    stage is exactly the number of funded ticks the parcel needs — counted here by the
///    model's own **repeated addition**, not by `ceil(package / net)`: 6,000 additions of
///    bloomcrown's increment give `0.049999999999996936`, which is below the package
///    `0.049999999999999996`, so the sixth-thousandth tick does *not* deliver and the
///    honest count is 6,001. [`ObservationCap::package_ticks`] keeps the closed form,
///    because that is the number R5.4 and R7.2 published; the difference is carried here
///    as [`ReplacementTimeline::closed_form_fund_ticks`] and printed.
/// 2. **Germinate.** A package that lands on tick `t` is born at tick `t + 1`: step 8 of
///    the next tick is the first lottery that can see it (`step`'s seed-bank doc). **One
///    tick**, and only if the site passes the predicate and wins its own local lottery.
/// 3. **Grow from `alive_min` to `donor_min`** under the growth cap: [`ObservationCap`]'s
///    own first term, unchanged.
/// 4. **Fund the descendant's own package**, which is stage 1 again for the new stand.
///
/// **It is a lower bound on a lower bound.** Every stage assumes perfect funding, an
/// immediate germination on a passing site, and income that keeps the growth cap saturated
/// the whole way — none of which the model promises. So a budget above this bound does not
/// make a replacement likely; a budget **below** it makes one impossible, which is the
/// thing R10.1 asks the harness to stop hiding.
#[derive(Clone, Copy, Debug)]
pub struct ReplacementTimeline {
    pub species: Species,
    /// Stage 1: funded ticks until the parcel holds one whole package, by repeated
    /// addition. The delivery happens on this tick.
    pub fund_ticks: u64,
    /// `ceil(package / net)`: what the published cap arithmetic uses. One tick less than
    /// `fund_ticks` when the repeated addition falls short of the package, which is the
    /// ordinary case.
    pub closed_form_fund_ticks: u64,
    /// Stage 2: one tick, the phase order's own gap between landing and the lottery.
    pub germinate_ticks: u64,
    /// Stage 3: the growth cap, `alive_min` → `donor_min`.
    pub grow_ticks: u64,
    /// Stage 4: the descendant funding its own package — stage 1 again.
    pub refund_ticks: u64,
    /// The tick the founder's first package leaves, counting the introduction as tick 0.
    pub deliver_tick: u64,
    /// The tick the newborn appears: the delivery tick plus the germination tick.
    pub birth_tick: u64,
    /// The tick that newborn reaches `donor_min`.
    pub donor_tick: u64,
    /// The tick the descendant's own package can leave — `total_ticks`. **One less than the
    /// four stages added up** (Astra R11.1): `step` grows in step 4 and propagates in step 9
    /// of the same tick, and `propagate` tests the wood that step 4 just produced, so the very
    /// tick that reaches `donor_min` can fund the first instalment of the descendant's own
    /// package. Stages 3 and 4 share that tick and adding them whole double-counts it.
    pub total_ticks: u64,
    pub total_s: f64,
}

/// One species' earliest-possible replacement timeline, from its own preset.
pub fn replacement_timeline(species: Species, sc: &SpeciesConfig) -> ReplacementTimeline {
    let dt = cubarium_voxel_flora::DT;
    let cap = observation_cap(species, sc);
    let package = cap.package;
    // The model's own two lines: `ask = propagule_rate · dt` gross out of the reserve, of
    // which `1 / (1 + build)` reaches the parcel.
    let net = sc.propagule_rate * dt / (1.0 + sc.build);
    let fund_ticks = ticks_to_fund(package, net);
    let germinate_ticks = 1;
    let grow_ticks = cap.growth_ticks;
    // The milestones, as ticks after the introduction. The last one **subtracts the tick
    // stages 3 and 4 share** (R11.1); an infinite stage keeps the whole timeline infinite,
    // because `saturating_add` stops at `u64::MAX` and the subtraction below is skipped.
    let deliver_tick = fund_ticks;
    let birth_tick = deliver_tick.saturating_add(germinate_ticks);
    let donor_tick = birth_tick.saturating_add(grow_ticks);
    let total_ticks = match donor_tick.saturating_add(fund_ticks) {
        u64::MAX => u64::MAX,
        sum => sum - 1,
    };
    let total_s = if total_ticks == u64::MAX {
        f64::INFINITY
    } else {
        total_ticks as f64 * dt
    };
    ReplacementTimeline {
        species,
        fund_ticks,
        closed_form_fund_ticks: cap.package_ticks,
        germinate_ticks,
        grow_ticks,
        refund_ticks: fund_ticks,
        deliver_tick,
        birth_tick,
        donor_tick,
        total_ticks,
        total_s,
    }
}

/// How many funded ticks a parcel needs to hold one whole `package`, **as the model adds
/// it**: `parcel += net` every tick, and the delivery test is `parcel < package`. Float
/// addition is not multiplication and this is the number that decides a tick.
///
/// `u64::MAX` for a species that can never fund one, and for a `net` so small that the
/// addition stops making progress at all — which is a real float condition and not a
/// hypothetical: `parcel + net == parcel` once `net` falls below the parcel's own epsilon.
fn ticks_to_fund(package: f64, net: f64) -> u64 {
    if !(package > 0.0) || !(net > 0.0) || !package.is_finite() || !net.is_finite() {
        return u64::MAX;
    }
    let mut parcel = 0.0f64;
    let mut ticks = 0u64;
    while parcel < package {
        let next = parcel + net;
        if next <= parcel {
            return u64::MAX; // the increment has vanished into the float
        }
        parcel = next;
        ticks += 1;
    }
    ticks
}

impl ReplacementTimeline {
    /// The four stages with their own numbers, so that a reader can add them up.
    pub fn line(&self) -> String {
        format!(
            "{:>14}: the first package leaves at tick {} ({:.2} s){}, the newborn appears at {}, it reaches donor size at {} (+{} growth ticks), and its own package can leave at **{} ticks, {:.2} s** — one tick less than the four stages added up, because the tick that reaches donor_min also funds the first instalment (R11.1)",
            self.species.name(),
            self.deliver_tick,
            self.deliver_tick as f64 * cubarium_voxel_flora::DT,
            if self.fund_ticks == self.closed_form_fund_ticks {
                String::new()
            } else {
                format!(
                    " [the closed form says {}; repeated addition falls short of the package on that tick]",
                    self.closed_form_fund_ticks
                )
            },
            self.birth_tick,
            self.donor_tick,
            self.grow_ticks,
            self.total_ticks,
            self.total_s
        )
    }
}

/// The **bound of a pair**: the larger of the two timelines, because one window has to be
/// able to resolve either direction.
pub fn pair_bound(a: &ReplacementTimeline, b: &ReplacementTimeline) -> f64 {
    a.total_s.max(b.total_s)
}

/// The cap of a **pair**, which is what an arm runs under: the larger of the two, so that
/// one window can resolve a replacement in either direction. Astra's R7.2 asked for a
/// species-appropriate cap rather than one number for every pair — 6,000 s was
/// bloomcrown's arithmetic being reused for stonecushion, which it cannot resolve.
pub fn pair_cap(a: &ObservationCap, b: &ObservationCap) -> f64 {
    a.total_s.max(b.total_s)
}

#[cfg(test)]
mod cap_tests {
    //! The observation cap's own arithmetic, against the two numbers Astra derived and
    //! against the model rule it claims to bound. Unit tests of this module, run by both
    //! examples that include it (`[[example]] test = true`).

    use super::*;
    use cubarium_voxel::{Material, World};
    use cubarium_voxel_flora::{Command, FloraConfig};

    /// **Astra's two numbers, to the hundredth of a second.** R5.4's bloomcrown pair —
    /// wood 0.02 to 0.3 at `wood_rate` 0.001 under the growth cap is at least 2,708.15 s,
    /// then 300 s to fund one package — and R7.2's stonecushion correction, 8,047 s of
    /// growth and 600 s for one 0.025 package, which is why a 6,000 s cap cannot resolve it.
    #[test]
    fn the_cap_arithmetic_reproduces_the_two_predeclared_numbers() {
        let config = FloraConfig::default();

        let bloom = observation_cap(Species::Bloomcrown, config.species(Species::Bloomcrown));
        assert_eq!(bloom.growth_ticks, 54_163);
        assert!(
            (bloom.growth_s - 2_708.15).abs() < 1e-9,
            "growth {}",
            bloom.growth_s
        );
        assert!(
            (bloom.package - 0.05).abs() < 1e-12,
            "package {}",
            bloom.package
        );
        assert!(
            (bloom.package_s - 300.0).abs() < 1e-9,
            "package {}",
            bloom.package_s
        );
        assert!(
            (bloom.total_s - 3_008.15).abs() < 1e-9,
            "total {}",
            bloom.total_s
        );

        let stone = observation_cap(Species::Stonecushion, config.species(Species::Stonecushion));
        assert_eq!(stone.growth_ticks, 160_945);
        assert!(
            (stone.growth_s - 8_047.25).abs() < 1e-9,
            "growth {}",
            stone.growth_s
        );
        assert!(
            (stone.package - 0.025).abs() < 1e-12,
            "package {}",
            stone.package
        );
        assert!(
            (stone.package_s - 600.0).abs() < 1e-9,
            "package {}",
            stone.package_s
        );
        assert!(
            (stone.total_s - 8_647.25).abs() < 1e-9,
            "total {}",
            stone.total_s
        );

        // The pair takes the larger, and 6,000 s resolves neither pair it is put against:
        // it is under bloomcrown's own 3,008.15 s twice over only for the original pair.
        let frond = observation_cap(
            Species::Umbrellafrond,
            config.species(Species::Umbrellafrond),
        );
        assert!((pair_cap(&bloom, &frond) - 3_008.15).abs() < 1e-9);
        assert!(
            pair_cap(&bloom, &stone) > 6_000.0,
            "6,000 s cannot resolve stonecushion"
        );
    }

    /// **The earliest-possible replacement timeline, stage by stage** (Astra R10.1). The
    /// published caps are *newborn* clocks and the study introduces a **founder with a zero
    /// parcel**, so the bound is one package earlier and one germination tick longer at each
    /// end.
    #[test]
    fn the_timeline_adds_the_two_stages_the_published_cap_leaves_out() {
        let config = FloraConfig::default();
        let bloom = replacement_timeline(Species::Bloomcrown, config.species(Species::Bloomcrown));
        // Stage 1 by repeated addition is **6,001** and not 6,000: 6,000 additions of
        // 8.3333e-6 give 0.049999999999996936 against the package's 0.049999999999999996.
        assert_eq!(bloom.closed_form_fund_ticks, 6_000);
        assert_eq!(
            bloom.fund_ticks, 6_001,
            "the model's own addition decides the tick"
        );
        assert_eq!(bloom.germinate_ticks, 1);
        assert_eq!(bloom.grow_ticks, 54_163);
        // Astra's own milestones (R11.1): delivered at 6,001, born at 6,002, donor size at
        // 60,165, and its own package can leave at 66,165 — **one less** than the four stages
        // added up, because the growth tick that reaches `donor_min` also funds the first
        // instalment (`step` grows in step 4 and propagates in step 9 of the same tick).
        assert_eq!(bloom.deliver_tick, 6_001);
        assert_eq!(bloom.birth_tick, 6_002);
        assert_eq!(bloom.donor_tick, 60_165);
        assert_eq!(bloom.total_ticks, 66_165);
        assert_eq!(
            bloom.total_ticks,
            6_001 + 1 + 54_163 + 6_001 - 1,
            "the shared tick"
        );
        assert!(
            (bloom.total_s - 3_308.25).abs() < 1e-9,
            "total {}",
            bloom.total_s
        );

        let stone =
            replacement_timeline(Species::Stonecushion, config.species(Species::Stonecushion));
        assert_eq!(stone.fund_ticks, 12_001);
        assert_eq!(stone.grow_ticks, 160_945);
        assert_eq!(
            (stone.deliver_tick, stone.birth_tick, stone.donor_tick),
            (12_001, 12_002, 172_947)
        );
        assert_eq!(stone.total_ticks, 184_947);
        assert!(
            (stone.total_s - 9_247.35).abs() < 1e-9,
            "total {}",
            stone.total_s
        );

        // The bound is strictly above the published cap, which is the whole of R10.1: a
        // budget at the published cap cannot observe the replacement it names.
        let cap = observation_cap(Species::Bloomcrown, config.species(Species::Bloomcrown));
        assert!(
            bloom.total_s > cap.total_s,
            "{} vs {}",
            bloom.total_s,
            cap.total_s
        );
        assert!((bloom.total_s - cap.total_s - 300.10).abs() < 1e-9);
    }

    /// **The whole timeline against the model, with the rates accelerated.** Four stages in
    /// one 300-tick fixture: the first package delivered, the germination one tick later, the
    /// newborn reaching `donor_min`, and that descendant funding a package of its own. Each
    /// measured stage is compared with its bound in the only direction a bound can be
    /// checked — the model is **never earlier** — and all four are required to happen, so a
    /// fixture that simply failed to reproduce could not pass.
    #[test]
    fn the_model_walks_the_whole_timeline_and_never_beats_it() {
        let mut world = World::empty(VoxelConfig {
            width: 4,
            height: 8,
            depth: 1,
            voxel_m: 1.0,
            seed: 11,
            ..VoxelConfig::default()
        });
        for x in 0..4i64 {
            for y in 1..=2u32 {
                let want = 0.6 * Material::Soil.pore_capacity() * world.config().voxel_volume();
                world.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z: 0,
                    volume_m3: want,
                });
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z: 0,
                    material: Material::Soil,
                });
            }
        }
        // Accelerated, and stated as such: a hundredfold assimilation, a rich pool, a fast
        // parcel and fast wood. Nothing here is a preset proposal — the point is to walk all
        // four stages inside a short test. The timeline's germination stage is the next
        // tick, so the bank is checked every tick and the seeds are dropped (package S's
        // fixture config).
        let mut config = FloraConfig {
            initial_mineral: 500.0,
            ..FloraConfig::default().drop_seeds_checked_each_tick()
        };
        {
            let sc = config.species_mut(Species::Bloomcrown);
            sc.assimilation = 40.0;
            sc.foliage_rate = 0.5;
            sc.propagule_rate = 0.12;
            sc.reserve_cap = 40.0;
            sc.wood_rate = 1.2;
            sc.hop = 1;
        }
        let timeline =
            replacement_timeline(Species::Bloomcrown, config.species(Species::Bloomcrown));
        let donor_min = config.species(Species::Bloomcrown).donor_min;
        let founder = founder_wood(config.species(Species::Bloomcrown));
        let mut flora = Flora::in_world(&world, config);
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 0,
                z: 0,
                species: Species::Bloomcrown,
                wood: founder
            }
        ));
        let founder_id = 0u64;

        let (mut delivered, mut born, mut grown, mut redelivered) = (None, None, None, None);
        let mut parcels: Vec<(u64, f64)> = vec![(founder_id, 0.0)];
        for tick in 1..=300u64 {
            flora.step(&mut world);
            let v = flora.view();
            if delivered.is_none()
                && v.ground
                    .iter()
                    .any(|g| g.seed_organic(Species::Bloomcrown) > 0.0)
            {
                delivered = Some(tick);
            }
            if born.is_none() && v.ledger.establishments > 0 {
                born = Some(tick);
            }
            for stand in v.stands.iter().filter(|s| s.id != founder_id) {
                if grown.is_none() && stand.wood >= donor_min {
                    grown = Some(tick);
                }
                let slot = parcels.iter().position(|(id, _)| *id == stand.id);
                match slot {
                    None => parcels.push((stand.id, stand.parcel)),
                    Some(i) => {
                        if redelivered.is_none() && stand.parcel < parcels[i].1 {
                            redelivered = Some(tick);
                        }
                        parcels[i].1 = stand.parcel;
                    }
                }
            }
        }

        let (d, b, g, r) = (
            delivered.expect("stage 1: no package was ever delivered"),
            born.expect("stage 2: nothing germinated"),
            grown.expect("stage 3: no descendant reached donor size"),
            redelivered.expect("stage 4: no descendant funded a package of its own"),
        );
        assert!(
            d >= timeline.fund_ticks,
            "stage 1 beat its bound: {d} < {}",
            timeline.fund_ticks
        );
        assert_eq!(
            b,
            d + timeline.germinate_ticks,
            "germination is the next tick, exactly"
        );
        assert!(
            g - b >= timeline.grow_ticks,
            "stage 3 beat the growth cap: {} ticks against {}",
            g - b,
            timeline.grow_ticks
        );
        // Stage 4 against **donor attainment**, not birth (R11.1): measuring it from the
        // birth tick folds the growth stage into it and cannot isolate the funding. The `+ 1`
        // is the shared tick — the donor tick itself can fund the first instalment.
        assert!(
            r - g + 1 >= timeline.refund_ticks,
            "stage 4 beat its bound: {} funded ticks from donor size against {}",
            r - g + 1,
            timeline.refund_ticks
        );
        assert!(
            r >= timeline.total_ticks,
            "the whole walk beat the timeline: {r} against {}",
            timeline.total_ticks
        );
    }

    /// **A species that cannot replace itself has no finite cap**, and the harness must
    /// print that rather than a number: a zero `wood_rate` never reaches `donor_min`, and a
    /// zero `propagule_rate` never funds a package.
    /// **An infinite stage keeps the whole timeline infinite**, and the shared-tick
    /// subtraction must not turn `u64::MAX` into a finite number one tick below it (R11.1).
    #[test]
    fn a_timeline_with_an_unreachable_stage_stays_infinite() {
        let mut config = FloraConfig::default();
        config.species_mut(Species::Bloomcrown).wood_rate = 0.0;
        let never = replacement_timeline(Species::Bloomcrown, config.species(Species::Bloomcrown));
        assert_eq!(never.grow_ticks, u64::MAX);
        assert_eq!(
            never.total_ticks,
            u64::MAX,
            "no off-by-one may make it finite"
        );
        assert!(never.total_s.is_infinite());

        let mut config = FloraConfig::default();
        config.species_mut(Species::Bloomcrown).propagule_rate = 0.0;
        let unfunded =
            replacement_timeline(Species::Bloomcrown, config.species(Species::Bloomcrown));
        assert_eq!(unfunded.fund_ticks, u64::MAX);
        assert_eq!(unfunded.total_ticks, u64::MAX);
        assert!(unfunded.total_s.is_infinite());
    }

    /// **Funding happens on the threshold-crossing tick itself** (R11.1): the tick whose
    /// growth reaches `donor_min` is the tick `propagate` reads that wood in, so the parcel
    /// starts filling immediately. This is the model claim the shared-tick subtraction rests
    /// on, and it is checked against the model and not asserted in a comment.
    #[test]
    fn a_stand_funds_on_the_very_tick_it_reaches_donor_size() {
        let mut world = World::empty(VoxelConfig {
            width: 3,
            height: 8,
            depth: 1,
            voxel_m: 1.0,
            seed: 13,
            ..VoxelConfig::default()
        });
        for x in 0..3i64 {
            for y in 1..=2u32 {
                let want = 0.6 * Material::Soil.pore_capacity() * world.config().voxel_volume();
                world.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z: 0,
                    volume_m3: want,
                });
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z: 0,
                    material: Material::Soil,
                });
            }
        }
        let mut config = FloraConfig {
            initial_mineral: 50.0,
            ..FloraConfig::default()
        };
        {
            let sc = config.species_mut(Species::Bloomcrown);
            sc.assimilation = 40.0;
            sc.wood_rate = 0.3; // 1.5 % of wood per tick: one tick crosses a 1 % gap
        }
        let donor_min = config.species(Species::Bloomcrown).donor_min;
        let mut flora = Flora::in_world(&world, config);
        // Just below the threshold, so the crossing is this fixture's own event.
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 1,
                z: 0,
                species: Species::Bloomcrown,
                wood: donor_min * 0.99
            }
        ));
        let before = *flora.view().stands.first().expect("the stand");
        assert!(before.wood < donor_min, "the premise: below donor_min");
        assert_eq!(before.parcel, 0.0, "and holding nothing");

        flora.step(&mut world);
        let after = *flora.view().stands.first().expect("still alive");
        assert!(
            after.wood >= donor_min,
            "the crossing tick: wood {}",
            after.wood
        );
        assert!(
            after.parcel > 0.0,
            "the crossing tick must also fund: parcel {} after wood {}",
            after.parcel,
            after.wood
        );
    }

    #[test]
    fn a_preset_that_cannot_replace_itself_has_an_infinite_cap() {
        let mut config = FloraConfig::default();
        config.species_mut(Species::Bloomcrown).wood_rate = 0.0;
        let cap = observation_cap(Species::Bloomcrown, config.species(Species::Bloomcrown));
        assert!(cap.growth_s.is_infinite() && cap.total_s.is_infinite());

        let mut config = FloraConfig::default();
        config.species_mut(Species::Bloomcrown).propagule_rate = 0.0;
        let cap = observation_cap(Species::Bloomcrown, config.species(Species::Bloomcrown));
        assert!(cap.package_s.is_infinite() && cap.total_s.is_infinite());
    }

    /// **The bound is one-sided, and the model obeys it.** The claim is that no stand can
    /// reach `donor_min` *before* `growth_ticks`, because `step.rs` allows at most
    /// `wood_rate · W · dt` of new wood per tick. Run it: a well-fed bloomcrown on wet soil
    /// in open sky with a hundredfold assimilation and an ample mineral pool, at a
    /// `wood_rate` fast enough to test in 181 ticks, is still **below** `donor_min` on the
    /// tick before the cap says it can be there.
    ///
    /// Only that direction is asserted. Reaching it *at* `growth_ticks` needs income to
    /// cover maintenance, both demands and the reserve every tick, which is exactly what the
    /// cap does not promise.
    #[test]
    fn the_model_cannot_reach_donor_size_sooner_than_the_cap_says() {
        let mut world = World::empty(VoxelConfig {
            width: 3,
            height: 8,
            depth: 1,
            voxel_m: 1.0,
            seed: 7,
            ..VoxelConfig::default()
        });
        for x in 0..3i64 {
            for y in 1..=2u32 {
                let want = 0.6 * Material::Soil.pore_capacity() * world.config().voxel_volume();
                world.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z: 0,
                    volume_m3: want,
                });
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z: 0,
                    material: Material::Soil,
                });
            }
        }

        let mut config = FloraConfig {
            initial_mineral: 50.0,
            ..FloraConfig::default()
        };
        {
            let sc = config.species_mut(Species::Bloomcrown);
            sc.wood_rate = 0.3;
            sc.assimilation = 0.4;
        }
        let cap = observation_cap(Species::Bloomcrown, config.species(Species::Bloomcrown));
        assert_eq!(cap.growth_ticks, 182, "the fixture's own arithmetic");

        let donor_min = config.species(Species::Bloomcrown).donor_min;
        let newborn = config.species(Species::Bloomcrown).alive_min;
        let mut flora = Flora::in_world(&world, config);
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 1,
                z: 0,
                species: Species::Bloomcrown,
                wood: newborn
            }
        ));
        for _ in 0..cap.growth_ticks - 1 {
            flora.step(&mut world);
        }
        let stand = *flora
            .view()
            .stands
            .first()
            .expect("the newborn is still alive");
        assert!(
            stand.wood < donor_min,
            "the growth cap was beaten: wood {} at tick {} against donor_min {donor_min}",
            stand.wood,
            cap.growth_ticks - 1
        );
        // And it is genuinely growing — a starved fixture would satisfy the bound for the
        // wrong reason. It multiplied its wood five and a half times in those 181 ticks and
        // still could not beat the cap, because income and not the cap is what bounds it
        // here: 0.1102 against the cap's own ceiling of 0.2960.
        assert!(
            stand.wood > 2.0 * newborn,
            "a starved fixture proves nothing: wood {}",
            stand.wood
        );
        let ceiling =
            newborn * (1.0 + 0.3 * cubarium_voxel_flora::DT).powi(cap.growth_ticks as i32 - 1);
        assert!(
            stand.wood <= ceiling + 1e-12,
            "wood {} over the cap's ceiling {ceiling}",
            stand.wood
        );
    }
}

// ======================================================== the conditioning decision
//
// Astra's R10.2: **elapsed conditioning is not a conditioned habitat.** The smoke conditioned
// for a fixed 1,000 s and then accepted whatever it had, and what it had was a storage rate of
// +0.0207 m³/s — 54 % of the accepted rain — beside a head that had stopped moving. A nearly
// stationary head is not a settled water table, and a settled water table is what the
// replacement comparison assumes when it calls two arms matched.
//
// So conditioning becomes a **decision with a budget**: named tolerances, tested over
// consecutive intervals, and "conditioning unresolved" printed when the budget runs out. The
// tolerances below are **placeholders and nothing measured them**; they are printed with every
// run and listed in `design/backlog.md` §1.

/// What one conditioning interval read: the rates the core's own ledger gives, the head, and
/// how much the eligible-site sets moved since the interval before.
#[derive(Clone, Debug)]
pub struct IntervalRecord {
    /// Seconds of conditioning elapsed at the **end** of this interval, **derived from the
    /// ticks actually executed** and not from the seconds requested (Astra R11.2): a chunk of
    /// 0.01 s rounds to zero ticks, and an observer that advanced by the request would then
    /// believe time had passed in which nothing ran.
    pub seconds: f64,
    /// Ticks this interval actually executed.
    pub ticks: u64,
    /// Whether that is a **whole** interval. A short final chunk reports smaller changes
    /// because less happened in it — a zero-tick tail reports *no* change at all — and would
    /// otherwise satisfy the tolerances by arithmetic rather than by settling, which is
    /// exactly how a 200.01 s budget could manufacture a settled phase (R11.2). An incomplete
    /// interval can never qualify.
    pub complete: bool,
    /// Accepted rain over the interval, m³/s: the denominator the storage tolerance is a
    /// fraction of, because a world that is taking no water in cannot be asked to hold its
    /// storage to a fraction of it.
    pub rain_rate: f64,
    /// Storage change over the interval, m³/s, signed.
    pub storage_rate: f64,
    pub head_m: f64,
    /// Head change over the interval, m, signed.
    pub head_delta: f64,
    /// Per species, how many skyline columns passed its establishment predicate at the end of
    /// the interval — reported, and not part of the decision.
    pub eligible: Vec<usize>,
    /// Per species, the **turnover** of its eligible set against the previous interval's:
    /// `|symmetric difference| / |union|`, so a set that gained and lost the same number of
    /// columns is not called stationary. `None` on the first interval, which therefore can
    /// never be a settled one.
    pub turnover: Vec<Option<f64>>,
}

impl IntervalRecord {
    /// The largest turnover any species showed, or `None` if this is the first interval.
    pub fn worst_turnover(&self) -> Option<f64> {
        self.turnover
            .iter()
            .copied()
            .try_fold(0.0f64, |a, t| Some(a.max(t?)))
    }
}

/// The settling tolerances, as **named placeholders**. Nothing measured any of them; they say
/// what this harness is prepared to call settled, and they are printed with every run.
#[derive(Clone, Copy, Debug)]
pub struct Tolerances {
    /// `|storage change| <= this fraction of the interval's own accepted rain`. 0.05 is
    /// "a twentieth of what came in is still moving into or out of store".
    pub storage_fraction_of_rain: f64,
    /// `|head change over one interval| <= this many metres`. 0.001 m is a millimetre of
    /// water table over 100 s.
    pub head_m_per_interval: f64,
    /// `eligible-set turnover <= this fraction of the union`. 0.02 is "the habitat one
    /// species could establish on stopped moving to within a fiftieth of itself".
    pub eligible_turnover: f64,
    /// How many **consecutive** intervals must satisfy all three. Two, so that one quiet
    /// interval in a drifting run cannot end conditioning.
    pub intervals: usize,
}

impl Default for Tolerances {
    fn default() -> Tolerances {
        Tolerances {
            storage_fraction_of_rain: 0.05,
            head_m_per_interval: 0.001,
            eligible_turnover: 0.02,
            intervals: 2,
        }
    }
}

impl Tolerances {
    pub fn line(&self) -> String {
        format!(
            "settling tolerances (**placeholders, nothing measured them**): |storage change| <= {:.0} % of the interval's own accepted rain, |head change| <= {} m per interval, eligible-set turnover <= {:.0} % of the union, all three on {} consecutive intervals",
            100.0 * self.storage_fraction_of_rain,
            self.head_m_per_interval,
            100.0 * self.eligible_turnover,
            self.intervals
        )
    }

    /// Whether one interval satisfies all three tolerances. The first interval of a phase has
    /// no turnover and can never satisfy them.
    pub fn holds(&self, r: &IntervalRecord) -> bool {
        if !r.complete {
            return false; // R11.2: a partial interval cannot qualify, whatever it read
        }
        let storage_ok = r.storage_rate.abs()
            <= self.storage_fraction_of_rain * r.rain_rate.abs().max(f64::MIN_POSITIVE);
        let head_ok = r.head_delta.abs() <= self.head_m_per_interval;
        let turnover_ok = r
            .worst_turnover()
            .is_some_and(|t| t <= self.eligible_turnover);
        storage_ok && head_ok && turnover_ok
    }

    /// Which tolerance a single interval failed, for the printed line.
    pub fn why(&self, r: &IntervalRecord) -> String {
        let mut out: Vec<String> = Vec::new();
        if !r.complete {
            out.push(format!(
                "the interval is **incomplete** ({} ticks) and cannot qualify however quiet it                  looks",
                r.ticks
            ));
        }
        let limit = self.storage_fraction_of_rain * r.rain_rate.abs().max(f64::MIN_POSITIVE);
        if r.storage_rate.abs() > limit {
            out.push(format!(
                "storage {:+.6} m3/s is {:.0} % of rain (limit {:.0} %)",
                r.storage_rate,
                100.0 * r.storage_rate.abs() / r.rain_rate.abs().max(f64::MIN_POSITIVE),
                100.0 * self.storage_fraction_of_rain
            ));
        }
        if r.head_delta.abs() > self.head_m_per_interval {
            out.push(format!(
                "head moved {:+.4} m (limit {} m)",
                r.head_delta, self.head_m_per_interval
            ));
        }
        match r.worst_turnover() {
            None => out.push("no previous interval to compare the eligible sets with".to_string()),
            Some(t) if t > self.eligible_turnover => out.push(format!(
                "eligible-set turnover {:.1} % (limit {:.0} %)",
                100.0 * t,
                100.0 * self.eligible_turnover
            )),
            Some(_) => {}
        }
        if out.is_empty() {
            "settled".to_string()
        } else {
            out.join("; ")
        }
    }
}

/// What a conditioning phase's records add up to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Settle {
    /// The tolerances held on the last `intervals` consecutive records, the last of which
    /// ended at this many seconds.
    Settled { at_s: f64 },
    /// Not yet, and there is budget left.
    Running,
    /// The budget ran out first. **"Conditioning unresolved"**, and not a conditioned state.
    Expired,
}

/// The decision itself, over a phase's interval records: a pure function of the records, the
/// tolerances and the budget, so that it can be tested on **synthetic** records instead of by
/// running another long smoke (R10.2).
///
/// `Settled` needs the tolerances to hold on the last `tol.intervals` records — the *last*
/// ones, because a phase that settled and then drifted is not settled now.
pub fn conditioning_verdict(
    records: &[IntervalRecord],
    tol: &Tolerances,
    budget_s: f64,
    interval_s: f64,
) -> Settle {
    let n = tol.intervals.max(1);
    if records.len() >= n && records[records.len() - n..].iter().all(|r| tol.holds(r)) {
        return Settle::Settled {
            at_s: records.last().map_or(0.0, |r| r.seconds),
        };
    }
    // **Expired when another whole interval no longer fits** (R11.2), not when the budget is
    // merely exhausted: a fractional tail cannot qualify, so a budget with only a tail left
    // has nothing further to offer this phase and the remainder is not worth running.
    let elapsed = records.last().map_or(0.0, |r| r.seconds);
    if elapsed + interval_s > budget_s + 1e-9 {
        Settle::Expired
    } else {
        Settle::Running
    }
}

/// `|symmetric difference| / |union|` of two sorted site sets: 0.0 for two identical sets,
/// 1.0 for two disjoint ones, and 0.0 for two empty ones (nothing moved).
pub fn turnover_of(before: &[Site], after: &[Site]) -> f64 {
    let mut union = 0usize;
    let mut both = 0usize;
    let (mut i, mut j) = (0usize, 0usize);
    while i < before.len() || j < after.len() {
        union += 1;
        match (before.get(i), after.get(j)) {
            (Some(a), Some(b)) if a == b => {
                both += 1;
                i += 1;
                j += 1;
            }
            (Some(a), Some(b)) if a < b => i += 1,
            (Some(_), Some(_)) => j += 1,
            (Some(_), None) => i += 1,
            (None, Some(_)) => j += 1,
            (None, None) => unreachable!(),
        }
    }
    if union == 0 {
        0.0
    } else {
        (union - both) as f64 / union as f64
    }
}

/// Every skyline column one species could establish on now, sorted: the set whose turnover
/// the conditioning decision reads, through the model's own predicate.
///
/// The sky cache is the caller's, so a phase that observes every interval pays for one
/// hemisphere ray per site and reuses it across the six species and across intervals while
/// the terrain is unchanged ([`FloraView::establishment_gates_over`]). Pore water,
/// saturation, standing water and the saprotroph's dead wood are read afresh every call.
pub fn eligible_sites(
    world: &World,
    flora: &Flora,
    skyline: &[Site],
    species: Species,
    sky: &mut SkyCache,
) -> Vec<Site> {
    let gates = flora
        .view()
        .establishment_gates_over(&world.view(), skyline, species, sky);
    let mut out: Vec<Site> = skyline
        .iter()
        .copied()
        .zip(gates)
        .filter(|(_, g)| g.passes())
        .map(|(s, _)| s)
        .collect();
    out.sort_unstable();
    out
}

#[cfg(test)]
mod settle_tests {
    //! Astra's R10.2 asked for the conditioning decision to be tested on **synthetic interval
    //! records** rather than by running another long smoke. These are those records: no world,
    //! no model, no water — just the decision.

    use super::*;

    /// One interval, with the rain fixed at the harness's own nominal 0.0384 m³/s so that the
    /// storage tolerance has its denominator.
    fn rec(seconds: f64, storage: f64, head_delta: f64, turnover: Option<f64>) -> IntervalRecord {
        IntervalRecord {
            seconds,
            ticks: 2_000,
            complete: true,
            rain_rate: 0.0384,
            storage_rate: storage,
            head_m: 2.5,
            head_delta,
            eligible: vec![100, 200],
            turnover: vec![turnover, turnover],
        }
    }

    /// A quiet interval: a thousandth of the rain into store, a tenth of a millimetre of head,
    /// half a per cent of turnover.
    fn quiet(seconds: f64) -> IntervalRecord {
        rec(seconds, 0.0000384, 0.0001, Some(0.005))
    }

    #[test]
    fn two_consecutive_quiet_intervals_settle_and_one_does_not() {
        let tol = Tolerances::default();
        assert_eq!(
            tol.intervals, 2,
            "the placeholder this test is written against"
        );
        assert_eq!(
            conditioning_verdict(&[quiet(100.0)], &tol, 1_000.0, 100.0),
            Settle::Running
        );
        assert_eq!(
            conditioning_verdict(&[quiet(100.0), quiet(200.0)], &tol, 1_000.0, 100.0),
            Settle::Settled { at_s: 200.0 }
        );
    }

    #[test]
    fn the_first_interval_of_a_phase_can_never_settle() {
        let tol = Tolerances::default();
        let first = rec(100.0, 0.0, 0.0, None);
        assert!(!tol.holds(&first), "no previous set to compare with");
        assert!(
            tol.why(&first).contains("no previous interval"),
            "{}",
            tol.why(&first)
        );
        assert_eq!(
            conditioning_verdict(&[first, quiet(200.0)], &tol, 1_000.0, 100.0),
            Settle::Running,
            "the pair is not two *holding* intervals"
        );
    }

    #[test]
    fn a_phase_that_settled_and_then_drifted_is_not_settled_now() {
        let tol = Tolerances::default();
        let drift = rec(300.0, 0.0207, 0.0001, Some(0.005)); // the smoke's own 54 % of rain
        let records = vec![quiet(100.0), quiet(200.0), drift];
        assert_eq!(
            conditioning_verdict(&records, &tol, 1_000.0, 100.0),
            Settle::Running
        );
    }

    #[test]
    fn each_tolerance_can_fail_on_its_own_and_says_which() {
        let tol = Tolerances::default();
        // The smoke's final interval, which the old harness accepted as conditioned.
        let storage = rec(1_000.0, 0.0207, 0.0001, Some(0.005));
        assert!(!tol.holds(&storage));
        assert!(
            tol.why(&storage).contains("54 % of rain"),
            "{}",
            tol.why(&storage)
        );

        let head = rec(1_000.0, 0.0000384, -0.0032, Some(0.005));
        assert!(!tol.holds(&head));
        assert!(tol.why(&head).contains("head moved"), "{}", tol.why(&head));

        let churn = rec(1_000.0, 0.0000384, 0.0001, Some(0.31));
        assert!(!tol.holds(&churn));
        assert!(
            tol.why(&churn).contains("turnover 31.0 %"),
            "{}",
            tol.why(&churn)
        );

        // And all three together pass, with the reason line saying so.
        let ok = quiet(1_000.0);
        assert!(tol.holds(&ok));
        assert_eq!(tol.why(&ok), "settled");
    }

    #[test]
    fn a_budget_that_runs_out_before_the_tolerances_hold_is_unresolved() {
        let tol = Tolerances::default();
        let records = vec![
            rec(100.0, 0.0207, -0.06, None),
            rec(200.0, 0.0195, -0.05, Some(0.3)),
        ];
        assert_eq!(
            conditioning_verdict(&records, &tol, 500.0, 100.0),
            Settle::Running
        );
        assert_eq!(
            conditioning_verdict(&records, &tol, 200.0, 100.0),
            Settle::Expired
        );
        // Expiry is about the budget and never about the tolerances being wrong.
        assert_eq!(conditioning_verdict(&[], &tol, 0.0, 100.0), Settle::Expired);
    }

    /// **The per-tolerance summary and the extrapolation**, on synthetic records: which
    /// tolerance was first met on which interval, which was never met and what it did instead,
    /// and where a decaying head is heading.
    #[test]
    fn a_phase_reports_each_tolerance_and_where_the_head_was_going() {
        let tol = Tolerances::default();
        // Storage quiet from the start, head decaying by half each interval, turnover settling
        // on the third: three different answers from one phase.
        let records = vec![
            rec(100.0, 0.0000384, -0.08, None),
            rec(200.0, 0.0000384, -0.04, Some(0.10)),
            rec(300.0, 0.0000384, -0.02, Some(0.01)),
        ];
        assert_eq!(tol.first_met(Which::Storage, &records), Some((1, 100.0)));
        assert_eq!(
            tol.first_met(Which::Head, &records),
            None,
            "0.02 m is still 20x the limit"
        );
        assert_eq!(tol.first_met(Which::Turnover, &records), Some((3, 300.0)));
        assert_eq!(tol.trend(Which::Head, &records), Some((0.08, 0.02)));

        // The head's own steps halve, so the remaining travel is one more step's worth: from
        // 2.5 m with a -0.02 m step and a ratio of 0.5, the limit is 2.5 - 0.02 = 2.48 m.
        let (ratio, limit) = head_asymptote(&records).expect("three decaying steps");
        assert!((ratio - 0.5).abs() < 1e-12, "ratio {ratio}");
        assert!((limit - 2.48).abs() < 1e-12, "limit {limit}");

        // Not decaying: no extrapolation rather than a wrong one.
        let steady = vec![
            rec(100.0, 0.0, -0.05, None),
            rec(200.0, 0.0, -0.05, Some(0.0)),
            rec(300.0, 0.0, -0.06, Some(0.0)),
        ];
        assert_eq!(
            head_asymptote(&steady),
            None,
            "a growing step extrapolates to nothing"
        );
        assert_eq!(
            head_asymptote(&records[..2]),
            None,
            "fewer than three records"
        );
        // An incomplete interval satisfies no single tolerance either.
        let mut partial = rec(400.0, 0.0, 0.0, Some(0.0));
        partial.complete = false;
        partial.ticks = 0;
        for which in Which::ALL {
            assert!(
                !tol.holds_one(which, &partial),
                "{} on a partial interval",
                which.name()
            );
        }
    }

    /// **The 200.01 s case** (Astra R11.2). A phase whose budget is a hair over two intervals
    /// used to be able to manufacture a settled verdict: a first record with no turnover, one
    /// genuinely quiet 100 s record, and then a **0.01 s tail** that runs zero ticks, reports
    /// zero storage change, zero head movement and zero turnover, and becomes the second
    /// "quiet" interval the rule asks for.
    ///
    /// Two things stop it now, and both are pinned here: an **incomplete** interval can never
    /// hold, whatever it read; and the verdict expires as soon as another *whole* interval no
    /// longer fits the budget, so the tail is never even run.
    #[test]
    fn a_fractional_tail_cannot_manufacture_a_settled_phase() {
        let tol = Tolerances::default();
        let first = rec(100.0, 0.0207, -0.06, None);
        let second = quiet(200.0);
        // The tail as the old code would have recorded it: zero ticks, nothing moved.
        let tail = IntervalRecord {
            seconds: 200.01,
            ticks: 0,
            complete: false,
            rain_rate: 0.0384,
            storage_rate: 0.0,
            head_m: 2.5,
            head_delta: 0.0,
            eligible: vec![100, 200],
            turnover: vec![Some(0.0), Some(0.0)],
        };
        assert!(
            !tol.holds(&tail),
            "a zero-tick interval reads as perfectly quiet and must still not qualify"
        );
        assert!(tol.why(&tail).contains("incomplete"), "{}", tol.why(&tail));
        assert_eq!(
            conditioning_verdict(&[first.clone(), second.clone(), tail], &tol, 200.01, 100.0),
            Settle::Expired,
            "the tail cannot be the second quiet interval"
        );
        // And the phase expires before the tail is reached at all: after two records, 200 s of
        // a 200.01 s budget is spent and another whole interval does not fit.
        assert_eq!(
            conditioning_verdict(&[first, second], &tol, 200.01, 100.0),
            Settle::Expired,
            "no whole interval fits in the remaining 0.01 s"
        );
        // With a budget that does fit a third whole interval, the same two records are still
        // running rather than expired: expiry is about the budget and not about the tail.
        let (first, second) = (rec(100.0, 0.0207, -0.06, None), quiet(200.0));
        assert_eq!(
            conditioning_verdict(&[first, second], &tol, 300.0, 100.0),
            Settle::Running
        );
    }

    #[test]
    fn turnover_is_the_symmetric_difference_over_the_union() {
        let s = |xs: &[u32]| -> Vec<Site> { xs.iter().map(|&x| Site { x, y: 2, z: 0 }).collect() };
        assert_eq!(turnover_of(&s(&[1, 2, 3]), &s(&[1, 2, 3])), 0.0);
        assert_eq!(
            turnover_of(&[], &[]),
            0.0,
            "two empty sets moved by nothing"
        );
        assert_eq!(
            turnover_of(&s(&[1, 2]), &s(&[3, 4])),
            1.0,
            "disjoint is total turnover"
        );
        // One column gained: union 4, both 3.
        assert!((turnover_of(&s(&[1, 2, 3]), &s(&[1, 2, 3, 4])) - 0.25).abs() < 1e-12);
        // One gained and one lost is **not** stationary, which a count comparison would miss.
        assert!((turnover_of(&s(&[1, 2, 3]), &s(&[1, 2, 4])) - 0.5).abs() < 1e-12);
    }
}

// ------------------------------------------- reading a phase's records after the fact

/// Which single tolerance a record satisfied, for the per-tolerance summary a conditioning
/// probe prints. Each is read **alone**: a phase settles only when all three hold together on
/// consecutive intervals, and these say which of them is the one still failing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Storage,
    Head,
    Turnover,
}

impl Which {
    pub const ALL: [Which; 3] = [Which::Storage, Which::Head, Which::Turnover];

    pub fn name(self) -> &'static str {
        match self {
            Which::Storage => "storage",
            Which::Head => "head",
            Which::Turnover => "turnover",
        }
    }
}

impl Tolerances {
    /// Whether one record satisfies **one** of the three tolerances, ignoring the others.
    /// An incomplete interval satisfies none of them (R11.2).
    pub fn holds_one(&self, which: Which, r: &IntervalRecord) -> bool {
        if !r.complete {
            return false;
        }
        match which {
            Which::Storage => {
                r.storage_rate.abs()
                    <= self.storage_fraction_of_rain * r.rain_rate.abs().max(f64::MIN_POSITIVE)
            }
            Which::Head => r.head_delta.abs() <= self.head_m_per_interval,
            Which::Turnover => r
                .worst_turnover()
                .is_some_and(|t| t <= self.eligible_turnover),
        }
    }

    /// The **first** record on which one tolerance held, and its 1-based interval number:
    /// what a probe reports per tolerance, and `None` for "never, in this phase".
    pub fn first_met(&self, which: Which, records: &[IntervalRecord]) -> Option<(usize, f64)> {
        records
            .iter()
            .enumerate()
            .find(|(_, r)| self.holds_one(which, r))
            .map(|(i, r)| (i + 1, r.seconds))
    }

    /// One tolerance's own value on the first and last complete record, for the trend line a
    /// probe prints when the tolerance was never met.
    pub fn trend(&self, which: Which, records: &[IntervalRecord]) -> Option<(f64, f64)> {
        let mut complete = records.iter().filter(|r| r.complete);
        let first = complete.next()?;
        let last = records.iter().filter(|r| r.complete).next_back()?;
        let value = |r: &IntervalRecord| match which {
            Which::Storage => r.storage_rate.abs(),
            Which::Head => r.head_delta.abs(),
            Which::Turnover => r.worst_turnover().unwrap_or(f64::NAN),
        };
        Some((value(first), value(last)))
    }
}

/// A **geometric extrapolation** of where the head is going, from the last three complete
/// intervals: the ratio of consecutive head steps, and the limit that ratio implies.
///
/// `(ratio, limit)`, or `None` when there are not three complete records, when the steps are
/// not decaying (`ratio >= 1`, which is not converging at all) or when the last step is zero
/// (already still). **It is an extrapolation and not a measurement**: it assumes the next step
/// is `ratio` times the last one for ever, which the model never promised, and it exists so
/// that a probe that does not settle can still say what it was heading for instead of only
/// that it had not arrived.
pub fn head_asymptote(records: &[IntervalRecord]) -> Option<(f64, f64)> {
    let complete: Vec<&IntervalRecord> = records.iter().filter(|r| r.complete).collect();
    if complete.len() < 3 {
        return None;
    }
    let (a, b, c) = (
        complete[complete.len() - 3],
        complete[complete.len() - 2],
        complete[complete.len() - 1],
    );
    let (d1, d2) = (b.head_delta, c.head_delta);
    if d1 == 0.0 || d2 == 0.0 {
        return None;
    }
    // Two estimates of the same ratio; the later pair is the one used, the earlier one only
    // tells a reader whether the decay is itself steady.
    let _earlier = d1 / a.head_delta;
    let ratio = d2 / d1;
    if !(ratio.is_finite() && ratio.abs() < 1.0) {
        return None;
    }
    // h_inf = h_last + d2 * r / (1 - r): the sum of the remaining geometric steps.
    Some((ratio, c.head_m + d2 * ratio / (1.0 - ratio)))
}
