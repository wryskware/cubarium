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
use cubarium_voxel_flora::{Flora, Site, Species, SpeciesConfig};

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
            pool.retain(|s| view.material_at(s.x as i64, s.y, s.z) != cubarium_voxel::Material::Soil);
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
            top: f64::from(stand.site.y) + sc.crown_height(stand.wood),
            radius: sc.crown_radius(stand.wood),
            foliage: stand.foliage,
        });
    }
    for f in planned {
        if view.stands.iter().any(|s| s.site.x == f.x && s.site.z == f.z) {
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
            top: f64::from(site.y) + sc.crown_height(wood),
            radius: sc.crown_radius(wood),
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
    let g = flora.view().establishment_gates(&world.view(), site, species);
    format!(
        "mean pore {} (>= {:.2}), saturated fraction {:.3} (<= {:.2}), water {:.3} m \
         (<= {:.2}), sky {:.3} (>= {:.2}), {} soil voxels, dead wood in the box {:.3} \
         (>= {:.3})",
        g.mean_pore.map_or_else(|| "none".to_string(), |m| format!("{m:.3}")),
        sc.establish_pore_min,
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
    let own_top = f64::from(site.y) + sc.crown_height(founder_wood(sc));
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
/// the probe cannot drift from it.
pub fn prepared_world(seed: u64, noise_seed: u64) -> World {
    let dry =
        VoxelConfig { seed, noise_seed, rain_m_per_s: HARNESS_RAIN_M_PER_S, ..VoxelConfig::default() };
    let basin_floor_m =
        World::new(dry.clone()).outlet_cell().map_or(0.0, |(_, y, _)| y as f64) * dry.voxel_m;
    let config = VoxelConfig { initial_aquifer_head_m: basin_floor_m + 1.0, ..dry };
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
    let ok: Vec<Site> =
        skyline.iter().copied().filter(|s| passes(world, flora, species, *s)).collect();
    order_for(world, flora, species, habitat_of(species), ok, &[])
}

pub fn step_coupled(flora: &mut Flora, world: &mut World, seconds: f64) {
    let ticks = (seconds * cubarium_voxel::TICK_HZ as f64).round() as u64;
    for _ in 0..ticks {
        world.step();
        flora.step(world);
    }
}

pub fn count(flora: &Flora, species: Species) -> usize {
    flora.view().stands.iter().filter(|s| s.species == species).count()
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
    let package = if sc.propagule_split[0] > 0.0 { sc.alive_min / sc.propagule_split[0] } else { 0.0 };
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
    if t.is_finite() && t >= 0.0 { t as u64 } else { u64::MAX }
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
    let total_ticks = fund_ticks
        .saturating_add(germinate_ticks)
        .saturating_add(grow_ticks)
        .saturating_add(fund_ticks);
    let total_s = if total_ticks == u64::MAX { f64::INFINITY } else { total_ticks as f64 * dt };
    ReplacementTimeline {
        species,
        fund_ticks,
        closed_form_fund_ticks: cap.package_ticks,
        germinate_ticks,
        grow_ticks,
        refund_ticks: fund_ticks,
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
            "{:>14}: fund+deliver the first package {} ticks ({:.2} s){} + germinate {} tick ({:.2} s) + grow newborn -> donor {} ticks ({:.2} s) + the descendant's own package {} ticks ({:.2} s) = **{} ticks, {:.2} s** at the earliest",
            self.species.name(),
            self.fund_ticks,
            self.fund_ticks as f64 * cubarium_voxel_flora::DT,
            if self.fund_ticks == self.closed_form_fund_ticks {
                String::new()
            } else {
                format!(
                    " [the closed form says {}; repeated addition falls short of the package on that tick]",
                    self.closed_form_fund_ticks
                )
            },
            self.germinate_ticks,
            self.germinate_ticks as f64 * cubarium_voxel_flora::DT,
            self.grow_ticks,
            self.grow_ticks as f64 * cubarium_voxel_flora::DT,
            self.refund_ticks,
            self.refund_ticks as f64 * cubarium_voxel_flora::DT,
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
        assert!((bloom.growth_s - 2_708.15).abs() < 1e-9, "growth {}", bloom.growth_s);
        assert!((bloom.package - 0.05).abs() < 1e-12, "package {}", bloom.package);
        assert!((bloom.package_s - 300.0).abs() < 1e-9, "package {}", bloom.package_s);
        assert!((bloom.total_s - 3_008.15).abs() < 1e-9, "total {}", bloom.total_s);

        let stone = observation_cap(Species::Stonecushion, config.species(Species::Stonecushion));
        assert_eq!(stone.growth_ticks, 160_945);
        assert!((stone.growth_s - 8_047.25).abs() < 1e-9, "growth {}", stone.growth_s);
        assert!((stone.package - 0.025).abs() < 1e-12, "package {}", stone.package);
        assert!((stone.package_s - 600.0).abs() < 1e-9, "package {}", stone.package_s);
        assert!((stone.total_s - 8_647.25).abs() < 1e-9, "total {}", stone.total_s);

        // The pair takes the larger, and 6,000 s resolves neither pair it is put against:
        // it is under bloomcrown's own 3,008.15 s twice over only for the original pair.
        let frond = observation_cap(Species::Umbrellafrond, config.species(Species::Umbrellafrond));
        assert!((pair_cap(&bloom, &frond) - 3_008.15).abs() < 1e-9);
        assert!(pair_cap(&bloom, &stone) > 6_000.0, "6,000 s cannot resolve stonecushion");
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
        assert_eq!(bloom.fund_ticks, 6_001, "the model's own addition decides the tick");
        assert_eq!(bloom.germinate_ticks, 1);
        assert_eq!(bloom.grow_ticks, 54_163);
        assert_eq!(bloom.total_ticks, 6_001 + 1 + 54_163 + 6_001);
        assert!((bloom.total_s - 3_308.30).abs() < 1e-9, "total {}", bloom.total_s);

        let stone =
            replacement_timeline(Species::Stonecushion, config.species(Species::Stonecushion));
        assert_eq!(stone.fund_ticks, 12_001);
        assert_eq!(stone.grow_ticks, 160_945);
        assert!((stone.total_s - 9_247.40).abs() < 1e-9, "total {}", stone.total_s);

        // The bound is strictly above the published cap, which is the whole of R10.1: a
        // budget at the published cap cannot observe the replacement it names.
        let cap = observation_cap(Species::Bloomcrown, config.species(Species::Bloomcrown));
        assert!(bloom.total_s > cap.total_s, "{} vs {}", bloom.total_s, cap.total_s);
        assert!((bloom.total_s - cap.total_s - 300.15).abs() < 1e-9);
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
                world.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: want });
                world.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
            }
        }
        // Accelerated, and stated as such: a hundredfold assimilation, a rich pool, a fast
        // parcel and fast wood. Nothing here is a preset proposal — the point is to walk all
        // four stages inside a short test.
        let mut config = FloraConfig { initial_mineral: 500.0, ..FloraConfig::default() };
        {
            let sc = config.species_mut(Species::Bloomcrown);
            sc.assimilation = 40.0;
            sc.foliage_rate = 0.5;
            sc.propagule_rate = 0.12;
            sc.reserve_cap = 40.0;
            sc.wood_rate = 1.2;
            sc.hop = 1;
        }
        let timeline = replacement_timeline(Species::Bloomcrown, config.species(Species::Bloomcrown));
        let donor_min = config.species(Species::Bloomcrown).donor_min;
        let founder = founder_wood(config.species(Species::Bloomcrown));
        let mut flora = Flora::in_world(&world, config);
        assert!(flora.apply(
            &world,
            Command::Seed { x: 0, z: 0, species: Species::Bloomcrown, wood: founder }
        ));
        let founder_id = 0u64;

        let (mut delivered, mut born, mut grown, mut redelivered) = (None, None, None, None);
        let mut parcels: Vec<(u64, f64)> = vec![(founder_id, 0.0)];
        for tick in 1..=300u64 {
            flora.step(&mut world);
            let v = flora.view();
            if delivered.is_none() && v.ground.iter().any(|g| g.seed_organic(Species::Bloomcrown) > 0.0) {
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
        assert!(d >= timeline.fund_ticks, "stage 1 beat its bound: {d} < {}", timeline.fund_ticks);
        assert_eq!(b, d + timeline.germinate_ticks, "germination is the next tick, exactly");
        assert!(
            g - b >= timeline.grow_ticks,
            "stage 3 beat the growth cap: {} ticks against {}",
            g - b,
            timeline.grow_ticks
        );
        assert!(
            r - b >= timeline.refund_ticks,
            "stage 4 beat its bound: {} ticks from birth against {}",
            r - b,
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
                world.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: want });
                world.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
            }
        }

        let mut config = FloraConfig { initial_mineral: 50.0, ..FloraConfig::default() };
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
            Command::Seed { x: 1, z: 0, species: Species::Bloomcrown, wood: newborn }
        ));
        for _ in 0..cap.growth_ticks - 1 {
            flora.step(&mut world);
        }
        let stand = *flora.view().stands.first().expect("the newborn is still alive");
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
        assert!(stand.wood > 2.0 * newborn, "a starved fixture proves nothing: wood {}", stand.wood);
        let ceiling = newborn * (1.0 + 0.3 * cubarium_voxel_flora::DT).powi(cap.growth_ticks as i32 - 1);
        assert!(stand.wood <= ceiling + 1e-12, "wood {} over the cap's ceiling {ceiling}", stand.wood);
    }
}
