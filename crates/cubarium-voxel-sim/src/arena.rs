//! **The static phase-one arena**: a 32 × 16 × 12 voxel world at 0.25 m/voxel, prepared
//! once from a layout seed, with a finite consumable resource layout and the founder body
//! placed off it.
//!
//! This is the fixture `design/voxel-senses-phase1-plan.md`'s "Frozen arena contract"
//! describes. Terrain and water are laid down by [`Arena::build`] and then never evolve:
//! the arena is run through [`Sim::new_static`], whose schedule has no `Water` and no
//! `Flora` leg. What still runs is the production fauna tick — the same `sys_fauna` the
//! live schedule runs — so an animal placed here ages, pays maintenance and dies by the
//! real rules, and P1-B/P1-C add the real motion, contacts, senses and feeding behind the
//! same interfaces.
//!
//! # The two layouts
//!
//! [`Founder::Blind`] gets **litter** deposited on the ground: the finite stock the litter
//! feeder will eat. [`Founder::Browser`] gets **springturf** stands, each with full
//! foliage: the finite stock the browser will crop. Both are finite because neither
//! decomposition nor growth runs in the static schedule. [`Arena::resource_stock`] reads
//! the live total, so a test can watch a bite lower it.
//!
//! # What P5-B changed (`design/handoffs/voxel-retrain-2026-09-22.md`, item 3)
//!
//! The mouth is a physical band `[0, 1.33·H]` over the standing surface now, and the old
//! browser crowns a voxel above the head were out of it. Every site holds a
//! [`PatchKind`]: the shredder's **litter**, **carrion** and a **glowcap cap** fruiting
//! on litter; the browser's bloomcrown **seedling** (a ground rosette no taller than
//! 0.125 m), an adult's **basal rosette** under its upper crown, and — Stage A only, to
//! be seen and not eaten — a **stripped** adult whose rosette is gone. Every scored patch
//! is in the band of every body size an episode samples (`tests/arena_band.rs`); the
//! stocks are the ones the layouts always carried. The arena gains a **0.125 m variant**
//! ([`ArenaGrid::Fine`]), the same layout at twice the resolution, and two **bystander
//! bodies** of the other lineage ([`ARENA_BYSTANDERS`]), because other bodies are normal
//! (D7). [`Arena::refound`] rebuilds the animal layer so an episode can give its founder
//! a sampled body (D11).
//!
//! # What P1-A does *not* do
//!
//! The placed founder is idle. It has a continuous pose and a support face, but no
//! forward/turn/feed resolution, no sensory sample and no field; those are P1-B and P1-C.
//! P1-A's acceptance is that the world holds still and the body does not.
//!
//! # What P1-C adds
//!
//! [`Sim::new_static`] — which [`Arena::into_sim`]
//! reaches — settles the arena's litter cue field before the first tick samples it, and
//! the static schedule's fauna leg steps the production tick with that field, so a driven
//! founder's `Chem(litter)` reads the prepared cue while taste remains tied to current
//! mouth contact.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, FaunaConfig, Founder, Pose, Senses};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, Species as Plant,
    Taken, Trophic,
};

use crate::Sim;

/// Arena width in voxels; `x` wraps.
pub const ARENA_WIDTH: u32 = 32;
/// Arena height in voxels.
pub const ARENA_HEIGHT: u32 = 16;
/// Arena depth in voxels.
pub const ARENA_DEPTH: u32 = 12;
/// Edge length of one voxel, metres.
pub const ARENA_VOXEL_M: f64 = 0.25;
/// The `y` of the all-soil ground every column supports at.
pub const GROUND_Y: u32 = 4;

/// Fraction of a void cell a prepared pond cell is filled to.
const POND_FILL: f64 = 0.8;
/// Organic matter in one Stage-A litter tile.
const LITTER_PER_TILE: f64 = 0.2;
/// Organic matter in Stage B's successor litter patch.
///
/// Stage B only means anything if the first patch actually runs out inside the horizon.
/// The blind founder bites `bite_per_s = 0.0005` organic per second at full effort, so
/// 0.015 is thirty seconds of uninterrupted feeding. The initial patch is half this
/// size so acquisition and depletion leave enough of the 120-second horizon for the
/// successor leg; the successor remains the larger confirmation patch.
///
/// The cue pays for it: emission is `min(litter / 0.05, 1)` cue units per second, so a
/// patch this size emits at 0.30 rather than the saturated 1.0 of a Stage-A tile. The
/// gradient is weaker, not absent, and it is the honest consequence of a patch a founder
/// can finish.
pub const REACQUISITION_LITTER_PER_PATCH: f64 = 0.015;
/// Stage B's initial patch is half the successor stock (P3-A).
pub const REACQUISITION_INITIAL_PATCH_FRACTION: f64 = 0.5;
/// Litter mineral fraction (a plant tissue's order of magnitude).
const LITTER_MINERAL_FRACTION: f64 = 0.02;
/// Litter retained-energy density, at the litter energy cap.
const LITTER_ENERGY_DENSITY: f64 = 2.0;
/// How many resource tiles a default layout lays.
const RESOURCE_TILES: usize = 6;

/// Foliage on one browser stand, whatever species and wood carry its crown.
///
/// Exactly what a half-grown springturf held before crown heights varied
/// (`alpha · W = 2.0 · 0.03`), so the browser's edible stock, its depletion timing and
/// the taste cue that reads the amount are all unchanged: the only thing the crown-height
/// mix moves is **where the crown is**. A taller stand is seeded with the wood its height
/// needs and then trimmed back to this.
pub const BROWSER_FOLIAGE_PER_STAND: f64 = 0.06;

/// The grid an arena is laid on (P5-B, decision D11's "0.125 m variant"). Both are the
/// **same physical arena** — 8 m around, 3 m deep, the ground's surface 1.25 m up — and
/// the same layout from the same seed: the plan is drawn on the 0.25 m lattice and a fine
/// arena lays each 0.25 m cell as the 2 × 2 fine cells under it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ArenaGrid {
    /// 0.25 m voxels: the arena every phase-one to P3 policy trained on.
    #[default]
    Standard,
    /// 0.125 m voxels: the `small` preset's cell size.
    Fine,
}

impl ArenaGrid {
    /// Fine cells per standard cell along one axis.
    fn scale(self) -> u32 {
        match self {
            ArenaGrid::Standard => 1,
            ArenaGrid::Fine => 2,
        }
    }

    pub fn voxel_m(self) -> f64 {
        ARENA_VOXEL_M / f64::from(self.scale())
    }

    pub fn width(self) -> u32 {
        ARENA_WIDTH * self.scale()
    }

    pub fn height(self) -> u32 {
        ARENA_HEIGHT * self.scale()
    }

    pub fn depth(self) -> u32 {
        ARENA_DEPTH * self.scale()
    }

    /// The `y` of the ground's support faces: the top of `GROUND_Y + 1` standard cells.
    pub fn ground_y(self) -> u32 {
        (GROUND_Y + 1) * self.scale() - 1
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ArenaGrid::Standard => "0.25m",
            ArenaGrid::Fine => "0.125m",
        }
    }
}

/// What one resource site of an arena holds (P5-B item 3). Each is food the founder's
/// **mouth band** `[0, 1.33·H]` reaches from the ground — row e of the retrain brief —
/// except [`PatchKind::Stripped`], which is laid to be seen and not eaten.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PatchKind {
    /// Shredder: a litter pool on the ground.
    Litter,
    /// Shredder: a carrion pool on the ground (decisions §3: the shredder's third food).
    Carrion,
    /// Shredder: a glowcap's cap tissue, standing on half the patch's stock as litter —
    /// a saprotroph fruits on the litter it lives on, and the litter carries the cue.
    Cap,
    /// Browser: a bloomcrown **seedling**, the ground rosette no taller than 0.125 m
    /// (decisions §5): its whole crown is in the band.
    Seedling,
    /// Browser: an adult bloomcrown's **basal rosette** in the band, its upper crown
    /// above it — the floor food of a grazed meadow.
    Rosette,
    /// Browser: an adult bloomcrown whose rosette has been **stripped**: only the upper
    /// crown is left, above every body's band. Stage A only, and never a scored patch.
    Stripped,
}

impl PatchKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PatchKind::Litter => "litter",
            PatchKind::Carrion => "carrion",
            PatchKind::Cap => "cap",
            PatchKind::Seedling => "seedling",
            PatchKind::Rosette => "rosette",
            PatchKind::Stripped => "stripped",
        }
    }
}

/// A private stream for the patch-kind draw, so the pond, the resource sites and the
/// start placement are drawn from exactly the numbers they were before: only what each
/// site holds moves. (The value is the old crown-height stream's: that draw is this one.)
const PATCH_KIND_SALT: u64 = 0x_C807_4E16_4854_5321;

/// A private stream for the bystanders, for the same reason.
const BYSTANDER_SALT: u64 = 0x_B157_A4DE_5EED_0002;

/// Bystander bodies an arena places (P5-B item 3, decision D7: other bodies are normal).
/// They are the **other** lineage, so they never eat the candidate's food and Stage B's
/// patch accounting stays exact, and they arrive the way the seeder's founders do
/// (`StartingStores::HUNGRY`). Placeholder, `design/backlog.md` §1.
pub const ARENA_BYSTANDERS: usize = 2;

/// Squared standard columns a bystander keeps from the start and from every resource,
/// so it is never placed in the candidate's mouth or on its food. Placeholder.
const BYSTANDER_CLEARANCE: i64 = 9;

/// The kind of each resource site in a layout, in the sorted resource order.
///
/// Deterministic per seed and **balanced by construction**, as the crown heights it
/// replaces were: the palette is cycled over the sites from a seeded rotation and then
/// shuffled, so a Stage-A layout carries every kind twice and a Stage-B pair two kinds.
/// Stage B's palette holds only kinds the band reaches — both patches are scored.
fn patch_kinds(
    founder: Founder,
    layout_seed: u64,
    kind: LayoutKind,
    sites: usize,
) -> Vec<PatchKind> {
    let palette: &[PatchKind] = match (founder, kind) {
        (Founder::Blind, _) => &[PatchKind::Litter, PatchKind::Carrion, PatchKind::Cap],
        (Founder::Browser, LayoutKind::StageA) => {
            &[PatchKind::Seedling, PatchKind::Rosette, PatchKind::Stripped]
        }
        (Founder::Browser, LayoutKind::Reacquisition(_)) => {
            &[PatchKind::Seedling, PatchKind::Rosette]
        }
    };
    let mut rng = Rng::new(layout_seed ^ PATCH_KIND_SALT);
    let rotation = rng.below(palette.len());
    let mut kinds: Vec<PatchKind> = (0..sites)
        .map(|i| palette[(i + rotation) % palette.len()])
        .collect();
    for i in (1..kinds.len()).rev() {
        kinds.swap(i, rng.below(i + 1));
    }
    kinds
}

/// One body an arena or a landscape places: its lineage, its support face and its
/// heading. The stores it arrives with are the caller's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub founder: Founder,
    pub site: Site,
    pub heading_rad: f64,
}

/// A fresh animal layer holding exactly `bodies`, **births off**, each introduced on its
/// own face with its own stores: the ids come back in `bodies` order, `None` where the
/// layer refused one. No controller is installed — a body without one rests until its
/// driver attaches one.
pub fn populate(
    world: &World,
    config: FaunaConfig,
    bodies: impl IntoIterator<Item = (Placement, cubarium_voxel_fauna::StartingStores)>,
) -> (Fauna, Vec<Option<u64>>) {
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    let ids = bodies
        .into_iter()
        .map(|(p, stores)| {
            fauna
                .apply(
                    world,
                    FaunaCommand::IntroduceFounderOnFace {
                        site: p.site,
                        founder: p.founder,
                        stores,
                        heading_rad: p.heading_rad,
                    },
                )
                .then(|| fauna.view().ledger.births - 1)
        })
        .collect();
    (fauna, ids)
}

/// Edible stock at `site` for `founder`'s mouth band standing at `standing_y` with the
/// **adult** body: the shredder's litter and carrion pools there and a glowcap's cap
/// layers in the band; the browser's vascular foliage layers in the band. An upper crown
/// above the band is not stock — it cannot be eaten from the ground.
pub fn edible_stock(
    founder: Founder,
    flora: &Flora,
    voxel_m: f64,
    standing_y: u32,
    site: Site,
) -> f64 {
    let fv = flora.view();
    let band = FaunaConfig::default()
        .founder(founder)
        .adult_body()
        .mouth_layers(standing_y, voxel_m);
    let pools = match founder {
        Founder::Blind => fv.ground_at(site).map_or(0.0, |g| g.litter + g.carrion),
        Founder::Browser => 0.0,
    };
    let stand = fv.stand_at(site).filter(|s| {
        let fungal = fv.config.species(s.species).trophic == Trophic::Saprotroph;
        fungal == (founder == Founder::Blind)
    });
    pools
        + stand.map_or(0.0, |s| {
            fv.layers(s)
                .filter(|l| band.contains(&l.cell))
                .map(|l| l.stock)
                .sum()
        })
}

/// How full an arena founder arrives: **half its structure and no reserve** (P2-C).
///
/// A founder introduced full has nowhere to put what it eats — settled intake can only
/// replace the upkeep it already burned — which caps every episode at
/// `0.25 + maintenance/reference` and leaves standing still within a hair of the best
/// attainable score (P2-B's measurement: per-episode maxima of exactly 0.3100 at 1,200
/// ticks and 0.3700 at 2,400). Half a body and an empty reserve is a whole
/// `body_reference` of headroom. The live schedule's introductions are untouched.
pub const FOUNDER_START: cubarium_voxel_fauna::StartingStores =
    cubarium_voxel_fauna::StartingStores::HUNGRY;

/// Deterministic scalar stream, splitmix64. No clock, no thread state.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }

    /// A uniform draw in `[0, 1)`, from the top 53 bits.
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A built arena and the founder it placed. Cloning it clones the three layers — the
/// fauna's controller table clones empty, as it always does.
#[derive(Clone)]
pub struct Arena {
    /// Which founder this arena feeds.
    pub founder: Founder,
    /// The layout seed the terrain, pond and resources were drawn from.
    pub layout_seed: u64,
    /// The cells it is laid on.
    pub grid: ArenaGrid,
    pub world: World,
    pub flora: Flora,
    pub fauna: Fauna,
    /// The finite resource sites, in column order.
    pub resources: Vec<Site>,
    /// What each of `resources` holds, in the same order.
    pub resource_kinds: Vec<PatchKind>,
    /// Where the founder starts and which way it faces, if the layout found a start.
    pub start: Option<Placement>,
    /// The bystander bodies' placements (the other lineage).
    pub bystanders: Vec<Placement>,
    /// The id of the idle founder body placed at build time, if it landed.
    pub animal_id: Option<u64>,
    /// The bystanders' ids in the current animal layer, in `bystanders` order.
    pub bystander_ids: Vec<u64>,
}

/// Stage B's two real, finite patches. The metadata is fixture-facing only: it never
/// enters an observation or controller and exists so a later evaluation can report
/// reacquisition without rediscovering a target from the world.
pub struct ReacquisitionArena {
    pub arena: Arena,
    /// The patch the founder starts within the ordinary Stage-A signal distance of.
    pub initial_patch: Site,
    /// A distinct finite patch left after the initial patch is depleted.
    pub successor_patch: Site,
}

impl ReacquisitionArena {
    /// Move the two-patch layout into the ordinary static simulator.
    pub fn into_arena(self) -> Arena {
        self.arena
    }

    /// Move the arena while retaining the two fixture sites for an evaluator's
    /// post-episode accounting. The sites remain outside the controller boundary.
    pub fn into_parts(self) -> (Arena, Site, Site) {
        (self.arena, self.initial_patch, self.successor_patch)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LayoutKind {
    StageA,
    Reacquisition(SuccessorBand),
}

/// How far Stage B places the successor patch from the initial one, in squared arena
/// columns (a column is [`ARENA_VOXEL_M`] = 0.25 m).
///
/// The band is a curriculum rung, not a tuning knob: it decides whether the founder can
/// sense the second patch from the first one at all. The initial patch, the founder's
/// start band and both stocks are the same in either.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SuccessorBand {
    /// 4 to under 6 columns, 1.0 m to under 1.5 m. Inside the blind founder's measured
    /// 1.5 m litter-cue reach and well inside the browser's 2.0 m cone, so a founder
    /// standing on its emptied patch can already sense where to go next.
    Near,
    /// 8 columns and out: 2.0 m to the strip's widest 4.85 m. The landed Stage-B task,
    /// where neither founder can sense the successor from the patch it just emptied.
    #[default]
    Landed,
}

impl SuccessorBand {
    /// The half-open squared-column band a successor site must fall in.
    pub fn bounds(self) -> (i64, i64) {
        match self {
            SuccessorBand::Near => (16, 36),
            SuccessorBand::Landed => (64, i64::MAX),
        }
    }

    pub fn contains(self, distance_squared: i64) -> bool {
        let (low, high) = self.bounds();
        distance_squared >= low && distance_squared < high
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SuccessorBand::Near => "near",
            SuccessorBand::Landed => "landed",
        }
    }
}

impl Arena {
    /// Build the frozen arena for `founder` from `layout_seed`, and place one idle founder
    /// body off the food. The world is not stepped: the pond is a direct `AddWater` and the
    /// ground is direct `SetMaterial`, so there is no settling cost and nothing to freeze
    /// afterwards.
    pub fn build(founder: Founder, layout_seed: u64) -> Arena {
        Self::build_on(founder, layout_seed, ArenaGrid::Standard)
    }

    /// [`Arena::build`] on `grid`: the same layout, laid on 0.25 m or 0.125 m cells.
    pub fn build_on(founder: Founder, layout_seed: u64, grid: ArenaGrid) -> Arena {
        Self::build_kind(founder, layout_seed, LayoutKind::StageA, None, grid).0
    }

    /// Build Stage B with the successor in `band` ([`SuccessorBand`]).
    pub fn build_reacquisition_in(
        founder: Founder,
        layout_seed: u64,
        band: SuccessorBand,
    ) -> ReacquisitionArena {
        Self::build_reacquisition_on(founder, layout_seed, band, ArenaGrid::Standard)
    }

    /// [`Arena::build_reacquisition_in`] on `grid`.
    pub fn build_reacquisition_on(
        founder: Founder,
        layout_seed: u64,
        band: SuccessorBand,
        grid: ArenaGrid,
    ) -> ReacquisitionArena {
        Self::build_reacquisition_kind(founder, layout_seed, band, None, grid)
    }

    /// Build Stage B's smallest real continuation task: one reachable finite patch and
    /// one distinct successor. Both use the normal flora deposits, fauna feeding and
    /// static sensory field; this type only records which physical sites form the two
    /// patches for an evaluator after the controller has acted.
    pub fn build_reacquisition(founder: Founder, layout_seed: u64) -> ReacquisitionArena {
        Self::build_reacquisition_in(founder, layout_seed, SuccessorBand::Landed)
    }

    /// Build the same Stage-B layout and start position as [`Self::build_reacquisition`],
    /// but set the signed turn from the founder's heading to its initial patch exactly.
    ///
    /// This is a fixture-only diagnostic seam for the balanced blind offset sweep. The
    /// heading changes the body's honest observations; the target and requested angle
    /// are not exposed to its controller.
    pub fn build_reacquisition_with_start_turn(
        founder: Founder,
        layout_seed: u64,
        turn_to_initial_rad: f64,
    ) -> ReacquisitionArena {
        assert!(
            turn_to_initial_rad.is_finite(),
            "the diagnostic start turn must be finite"
        );
        Self::build_reacquisition_kind(
            founder,
            layout_seed,
            SuccessorBand::Landed,
            Some(turn_to_initial_rad),
            ArenaGrid::Standard,
        )
    }

    fn build_reacquisition_kind(
        founder: Founder,
        layout_seed: u64,
        band: SuccessorBand,
        start_turn_override: Option<f64>,
        grid: ArenaGrid,
    ) -> ReacquisitionArena {
        let (arena, initial_patch) = Self::build_kind(
            founder,
            layout_seed,
            LayoutKind::Reacquisition(band),
            start_turn_override,
            grid,
        );
        let initial_patch = initial_patch.expect("the two-patch layout records its initial patch");
        let successor_patch = arena
            .resources
            .iter()
            .copied()
            .find(|site| *site != initial_patch)
            .expect("the two-patch layout records its successor patch");
        ReacquisitionArena {
            arena,
            initial_patch,
            successor_patch,
        }
    }

    /// The layout is **planned on the 0.25 m lattice** — pond, resource sites, start and
    /// heading, from the same draws in the same order the arena has always made — and
    /// then **laid** on `grid`, each planned cell becoming the `scale × scale` cells under
    /// it. So a fine arena is the standard arena's layout at twice the resolution, and a
    /// standard arena's pond, sites and start are exactly what they were before P5-B.
    fn build_kind(
        founder: Founder,
        layout_seed: u64,
        kind: LayoutKind,
        start_turn_override: Option<f64>,
        grid: ArenaGrid,
    ) -> (Arena, Option<Site>) {
        let scale = grid.scale();
        let ground_y = grid.ground_y();
        let config = VoxelConfig {
            width: grid.width(),
            height: grid.height(),
            depth: grid.depth(),
            voxel_m: grid.voxel_m(),
            seed: layout_seed,
            ..VoxelConfig::default()
        };
        let mut world = World::empty(config);
        world.apply(WorldCommand::SetOutlet { open: false });

        // Ground: soil 1..=ground_y, bedrock already at 0. Wrap the x walk so the seam
        // column is an ordinary interior column.
        for z in 0..grid.depth() {
            for x in 0..i64::from(grid.width()) {
                for y in 1..=ground_y {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        // A planned 0.25 m cell `(x, z)` laid on the grid: its lowest-corner fine cell.
        let laid = |site: Site| Site {
            x: site.x * scale,
            y: ground_y,
            z: site.z * scale,
        };

        let mut rng = Rng::new(layout_seed);
        let voxel_volume = grid.voxel_m().powi(3);
        let mut pond = vec![false; (ARENA_WIDTH * ARENA_DEPTH) as usize];
        // A shallow prepared pond: a rock-free patch of ground with free water standing on
        // it, the same 0.2 m deep on either grid. One direct write, never solved, and the
        // static schedule never moves it.
        let px = rng.below(ARENA_WIDTH as usize) as i64;
        let pz = rng.below(ARENA_DEPTH as usize) as u32;
        for dz in 0..2u32 {
            for dx in 0..4i64 {
                let x = px + dx;
                let z = pz + dz;
                if z >= ARENA_DEPTH {
                    continue;
                }
                let idx =
                    (x.rem_euclid(ARENA_WIDTH as i64) as usize) * ARENA_DEPTH as usize + z as usize;
                pond[idx] = true;
                for fz in 0..scale {
                    for fx in 0..i64::from(scale) {
                        for fy in 1..=scale {
                            let _ = world.apply(WorldCommand::AddWater {
                                x: x * i64::from(scale) + fx,
                                y: ground_y + fy,
                                z: z * scale + fz,
                                volume_m3: POND_FILL * voxel_volume,
                            });
                        }
                    }
                }
            }
        }

        let mut flora = Flora::new(FloraConfig::for_voxel_size(grid.voxel_m()).without_graze_refuge());

        // Candidate support faces: soil ground, not the pond, and — unless this layout is
        // the seam layout — at least two columns off the wrapped seam, so a default layout
        // does not accidentally test wrap geometry. Planned on the 0.25 m lattice.
        let seam_layout = layout_seed % 4 == 3;
        let mut candidates: Vec<Site> = Vec::new();
        for z in 0..ARENA_DEPTH {
            for x in 0..ARENA_WIDTH as i64 {
                let idx = (x as usize) * ARENA_DEPTH as usize + z as usize;
                if pond[idx] {
                    continue;
                }
                if !seam_layout && (x < 2 || x >= ARENA_WIDTH as i64 - 2) {
                    continue;
                }
                candidates.push(Site {
                    x: x as u32,
                    y: GROUND_Y,
                    z,
                });
            }
        }
        // A seam layout lays its first tile on the wrapped seam column. The seam site is
        // drawn from the **same** dry candidate pool and removed from it before the other
        // picks, so a resource site can never be selected twice (the flaw that made seed
        // 95 lay `(0,4,8)` twice). Every later pick is also a `swap_remove` off `candidates`.
        let (mut resources, initial_patch) = match kind {
            LayoutKind::StageA => {
                let mut resources: Vec<Site> = Vec::new();
                if seam_layout {
                    let seam: Vec<Site> = candidates.iter().copied().filter(|s| s.x == 0).collect();
                    if !seam.is_empty() {
                        let pick = seam[rng.below(seam.len())];
                        let at = candidates
                            .iter()
                            .position(|s| *s == pick)
                            .expect("the seam candidate is in the pool");
                        candidates.swap_remove(at);
                        resources.push(pick);
                    }
                }
                let want = RESOURCE_TILES
                    .saturating_sub(resources.len())
                    .min(candidates.len());
                for _ in 0..want {
                    let pick = rng.below(candidates.len());
                    resources.push(candidates.swap_remove(pick));
                }
                (resources, None)
            }
            LayoutKind::Reacquisition(band) => {
                // The first patch uses the same dry support pool as Stage A. Its successor
                // sits in the requested band — landed's 2 m and out, or the curriculum's
                // near ring — leaving a genuine reacquisition leg without manufacturing a
                // route or a movement rule.
                let initial_at = rng.below(candidates.len());
                let initial = candidates.swap_remove(initial_at);
                let successors: Vec<Site> = candidates
                    .iter()
                    .copied()
                    .filter(|site| band.contains(arena_distance_squared(*site, initial)))
                    .collect();
                assert!(
                    !successors.is_empty(),
                    "{founder:?} seed {layout_seed}: the {} band has no dry support face \
                     left of the initial patch",
                    band.as_str()
                );
                let successor = successors[rng.below(successors.len())];
                let at = candidates
                    .iter()
                    .position(|site| *site == successor)
                    .expect("the successor remains in the dry candidate pool");
                candidates.swap_remove(at);
                (vec![initial, successor], Some(initial))
            }
        };
        resources.sort();

        // What each site holds, in the sorted resource order, drawn from its own stream
        // so the pond, the resource sites and the start placement are untouched.
        let kinds = patch_kinds(founder, layout_seed, kind, resources.len());
        for (index, &planned) in resources.iter().enumerate() {
            // The stock a scored patch carries: Stage A's tile, or Stage B's patch with
            // the initial one at half the successor.
            let full = match (founder, kind) {
                (Founder::Blind, LayoutKind::StageA) => LITTER_PER_TILE,
                (Founder::Blind, LayoutKind::Reacquisition(_)) => REACQUISITION_LITTER_PER_PATCH,
                (Founder::Browser, _) => BROWSER_FOLIAGE_PER_STAND,
            };
            let organic = if Some(planned) == initial_patch {
                REACQUISITION_INITIAL_PATCH_FRACTION * full
            } else {
                full
            };
            lay_patch(&mut flora, &world, laid(planned), kinds[index], organic);
        }

        // Stage A begins off food but already inside a useful signal. Pick a dry support
        // two to four columns from a resource. Two columns keeps the mouth out of feeding
        // contact; four keeps low foliage large enough for the deliberately sparse ray
        // fan and remains inside the settled litter gradient. The pool and pick remain
        // seed-deterministic.
        //
        // The signal is the **food's**: a stripped adult is not a start target (P5-B
        // repair). Aimed at the nearest site of any kind, seed 6's browser started two
        // columns from a stripped crown with no edible patch it could see in a whole
        // turn on the spot.
        let edible: Vec<Site> = resources
            .iter()
            .zip(&kinds)
            .filter(|(_, k)| **k != PatchKind::Stripped)
            .map(|(r, _)| *r)
            .collect();
        let start_targets: &[Site] = initial_patch
            .as_ref()
            .map_or(edible.as_slice(), std::slice::from_ref);
        // The near band puts the successor within 6 columns of the initial patch, which
        // is inside the 2-to-4-column ring the start is drawn from: without this the
        // founder could be placed in feeding contact with the patch it is supposed to
        // have to find. The landed band satisfies the same rule for free — its patches
        // are 8 columns apart and the start is at most 4 from the initial one — so it is
        // scoped to `Near` and the landed pool is left exactly as it was. Stage A keeps
        // the same two columns from **every** site, stripped crowns included, now that
        // those are no longer targets that already kept the start off them.
        let start_clearance = match kind {
            LayoutKind::Reacquisition(SuccessorBand::Near) | LayoutKind::StageA => 4,
            LayoutKind::Reacquisition(SuccessorBand::Landed) => 0,
        };
        let mut starts: Vec<(Site, Site, i64)> = candidates
            .iter()
            .copied()
            .filter(|s| !resources.contains(s))
            .filter(|s| {
                resources
                    .iter()
                    .all(|r| arena_distance_squared(*s, *r) >= start_clearance)
            })
            .filter_map(|s| {
                start_targets
                    .iter()
                    .copied()
                    .map(|r| (r, arena_distance_squared(s, r)))
                    .min_by_key(|(_, distance)| *distance)
                    .filter(|(_, distance)| (4..=16).contains(distance))
                    .map(|(target, distance)| (s, target, distance))
            })
            .collect();
        starts.sort_by_key(|(site, target, distance)| (*distance, *site, *target));
        let start = (!starts.is_empty()).then(|| starts[rng.below(starts.len())]);

        let start = start.map(|(site, target, _)| {
            // The start heading (P2-B step 2). Phase one aimed both founders at the
            // in-signal resource with a +/-5 degree jitter, which made "go forward and
            // keep feeding" a winning open-loop policy and left sensing untested. The
            // heading is now drawn from the layout seed: uniform over the circle for the
            // blind founder, uniform within a half-turn of the bearing for the browser.
            // Only arena construction ever sees the target; no runtime path does.
            let dx = wrapped_dx(site.x, target.x) as f64;
            let dz = f64::from(target.z) - f64::from(site.z);
            let toward = dx.atan2(dz);
            let heading_rad = if let Some(turn) = start_turn_override {
                toward - turn
            } else {
                match founder {
                    // The blind founder is handed nothing: its heading is uniform over the
                    // whole circle, drawn from the layout seed. Whatever it finds, it finds
                    // by smelling and walking.
                    Founder::Blind => rng.unit() * std::f64::consts::TAU,
                    // The browser's foliage must be *somewhere it could look*, not straight
                    // ahead: uniform within +/-90 degrees of the bearing to the target, so
                    // the target lies inside the -90..+90 the three sectors cover at the
                    // first sample and is usually well off centre.
                    Founder::Browser => {
                        toward + (2.0 * rng.unit() - 1.0) * std::f64::consts::FRAC_PI_2
                    }
                }
            };
            // In the fauna's own `[0, 2pi)`, which its motion step wraps every heading
            // into, so an idle body's pose is the pose it was placed with.
            Placement {
                founder,
                site,
                heading_rad: heading_rad.rem_euclid(std::f64::consts::TAU),
            }
        });

        // Bystanders (D7): the other lineage, on dry ground clear of the start and the
        // food, from their own stream.
        let other = match founder {
            Founder::Blind => Founder::Browser,
            Founder::Browser => Founder::Blind,
        };
        let mut brng = Rng::new(layout_seed ^ BYSTANDER_SALT);
        let mut pool: Vec<Site> = candidates
            .iter()
            .copied()
            .filter(|s| !resources.contains(s))
            .filter(|s| {
                resources
                    .iter()
                    .chain(start.as_ref().map(|p| &p.site))
                    .all(|r| arena_distance_squared(*s, *r) >= BYSTANDER_CLEARANCE)
            })
            .collect();
        pool.sort();
        let mut bystanders = Vec::new();
        for _ in 0..ARENA_BYSTANDERS.min(pool.len()) {
            let site = pool.swap_remove(brng.below(pool.len()));
            bystanders.push(Placement {
                founder: other,
                site: laid(site),
                heading_rad: brng.unit() * std::f64::consts::TAU,
            });
        }
        let start = start.map(|p| Placement {
            site: laid(p.site),
            ..p
        });

        let mut arena = Arena {
            founder,
            layout_seed,
            grid,
            world,
            flora,
            fauna: Fauna::new(FaunaConfig::default()),
            resources: resources.iter().map(|&r| laid(r)).collect(),
            resource_kinds: kinds,
            start,
            bystanders,
            animal_id: None,
            bystander_ids: Vec::new(),
        };
        arena.refound(FOUNDER_START);
        let initial_patch = initial_patch.map(laid);
        (arena, initial_patch)
    }

    /// Rebuild the animal layer from the arena's placements: the founder at its start
    /// with `stores`, and the bystanders hungry, **births off** (plan, "Frozen arena
    /// contract"). What an episode calls to give its founder a sampled body (D11); the
    /// build calls it with [`FOUNDER_START`]. No controller is installed.
    pub fn refound(&mut self, stores: cubarium_voxel_fauna::StartingStores) {
        let bodies = self.start.map(|p| (p, stores)).into_iter().chain(
            self.bystanders
                .iter()
                .map(|p| (*p, cubarium_voxel_fauna::StartingStores::HUNGRY)),
        );
        let (fauna, ids) = populate(&self.world, FaunaConfig::default(), bodies);
        let mut ids = ids.into_iter();
        self.animal_id = if self.start.is_some() {
            ids.next().flatten()
        } else {
            None
        };
        self.bystander_ids = ids.flatten().collect();
        self.fauna = fauna;
    }

    /// The face the founder starts on, if the layout found one.
    pub fn start_face(&self) -> Option<Site> {
        self.start.map(|p| p.site)
    }

    /// The live edible stock in the resource layout ([`edible_stock`] summed over it).
    /// This is what a bite lowers.
    pub fn resource_stock(&self) -> f64 {
        self.resources
            .iter()
            .map(|&site| self.patch_stock(site))
            .sum()
    }

    /// The live edible stock on **one** site of this arena's resource layout, for the
    /// adult band from the ground ([`edible_stock`]). The Stage-B evaluator's per-patch
    /// reading; like [`Arena::resource_stock`] it is fixture surface, never an
    /// observation or a reward.
    pub fn patch_stock(&self, site: Site) -> f64 {
        edible_stock(
            self.founder,
            &self.flora,
            self.grid.voxel_m(),
            self.grid.ground_y(),
            site,
        )
    }

    /// The placed founder's pose, if it landed.
    pub fn animal_pose(&self) -> Option<Pose> {
        self.animal_id
            .and_then(|id| self.fauna.view().animal(id))
            .map(|a| a.pose)
    }

    /// Take up to `want` of the edible stock off one resource site, through the plant
    /// layer's production withdrawals — the ones the founder's mouth makes: litter, then
    /// carrion, then cap tissue in the band for the shredder; foliage in the band for the
    /// browser. `None` when the site holds nothing edible.
    pub fn take(&mut self, site: Site, want: f64) -> Option<Taken> {
        let band = FaunaConfig::default()
            .founder(self.founder)
            .adult_body()
            .mouth_layers(self.grid.ground_y(), self.grid.voxel_m());
        fn add(got: &mut Taken, t: Option<Taken>) {
            if let Some(t) = t {
                got.organic += t.organic;
                got.mineral += t.mineral;
                got.energy += t.energy;
            }
        }
        let mut got = Taken {
            organic: 0.0,
            mineral: 0.0,
            energy: 0.0,
        };
        match self.founder {
            Founder::Blind => {
                add(&mut got, self.flora.take_litter(site, want));
                let left = want - got.organic;
                if left > 0.0 {
                    add(&mut got, self.flora.take_carrion(site, left));
                }
                let left = want - got.organic;
                if left > 0.0 {
                    let t = self.flora.take_foliage_in_layers(site, left, &band);
                    add(&mut got, t.map(|t| t.taken));
                }
            }
            Founder::Browser => {
                let t = self.flora.take_foliage_in_layers(site, want, &band);
                add(&mut got, t.map(|t| t.taken));
            }
        }
        (got.organic > 0.0).then_some(got)
    }

    /// Settle a reusable cue field for this exact frozen source layout. Episode-private
    /// trend history is still reset when the field enters a simulation.
    ///
    /// The `Light` memo is held and warmed here too: the arena's terrain is frozen, so
    /// every episode's clone starts with every face's sky read.
    pub fn prepare_senses(&self) -> Senses {
        let mut senses = Senses::new();
        senses.settle(&self.world.view(), &self.flora.view());
        senses.hold_light();
        senses.warm_light(&self.world.view());
        senses
    }

    /// Move the built layers into a [`Sim`] in [`crate::ScheduleMode::Static`].
    pub fn into_sim(self, config: crate::SimConfig) -> Sim {
        Sim::new_static(self.world, self.flora, self.fauna, config)
    }

    /// Move the built layers into a static [`Sim`] with a field already settled for this
    /// arena's unchanged source layout.
    pub fn into_sim_prepared(self, config: crate::SimConfig, senses: Senses) -> Sim {
        Sim::new_static_prepared(self.world, self.flora, self.fauna, config, senses)
    }
}

/// Lay one patch of `kind` holding `organic` of edible stock on `site`.
fn lay_patch(flora: &mut Flora, world: &World, site: Site, kind: PatchKind, organic: f64) {
    let pool = |flora: &mut Flora, deposit: DepositKind, organic: f64| {
        let accepted = flora.deposit(
            site,
            Deposit {
                kind: deposit,
                organic,
                mineral: organic * LITTER_MINERAL_FRACTION,
                energy: organic * LITTER_ENERGY_DENSITY,
            },
        );
        debug_assert!(accepted, "a deposit on a support face");
    };
    let seed = |flora: &mut Flora, species: Plant, wood: f64| {
        let accepted = flora.apply(
            world,
            FloraCommand::Seed {
                x: i64::from(site.x),
                z: site.z,
                species,
                wood,
            },
        );
        assert!(
            accepted,
            "{species:?} seeded on the arena's ground at {site:?}"
        );
    };
    // The lowest foliage layer's cell of the stand on `site`: a rosette's.
    let lowest = |flora: &Flora| -> i64 {
        flora
            .view()
            .layers_at(site)
            .map(|l| l.cell)
            .min()
            .expect("a seeded stand has a foliage layer")
    };
    // Trim a stand's layers in `cells` down to `keep`.
    let trim = |flora: &mut Flora, cells: std::ops::RangeInclusive<i64>, keep: f64| {
        let held: f64 = flora
            .view()
            .layers_at(site)
            .filter(|l| cells.contains(&l.cell))
            .map(|l| l.stock)
            .sum();
        assert!(
            held >= keep - 1e-15,
            "{kind:?} at {site:?} holds {held} in {cells:?}, less than the patch's {keep}"
        );
        if held > keep {
            let taken = flora
                .take_foliage_in_layers(site, held - keep, &cells)
                .expect("the seeded stand has foliage to trim");
            debug_assert!((taken.taken.organic - (held - keep)).abs() < 1e-12);
        }
    };
    let config = flora.config().clone();
    match kind {
        PatchKind::Litter => pool(flora, DepositKind::Litter, organic),
        PatchKind::Carrion => pool(flora, DepositKind::Carrion, organic),
        PatchKind::Cap => {
            // Half the stock is the glowcap's cap, half the litter it fruits on.
            seed(flora, Plant::Glowcap, config.glowcap.wood_max);
            let cap = lowest(flora);
            trim(flora, cap..=cap, 0.5 * organic);
            pool(flora, DepositKind::Litter, 0.5 * organic);
        }
        PatchKind::Seedling => {
            // Decisions §5's seedling stage is `wood / wood_max <= 0.2`.
            seed(flora, Plant::Bloomcrown, 0.2 * config.bloomcrown.wood_max);
            trim(flora, i64::MIN..=i64::MAX, organic);
        }
        PatchKind::Rosette => {
            seed(flora, Plant::Bloomcrown, config.bloomcrown.wood_max);
            let rosette = lowest(flora);
            trim(flora, rosette..=rosette, organic);
        }
        PatchKind::Stripped => {
            seed(flora, Plant::Bloomcrown, config.bloomcrown.wood_max);
            let rosette = lowest(flora);
            trim(flora, rosette..=rosette, 0.0);
        }
    }
}

/// Shortest signed x-column displacement on the wrapped arena strip.
fn wrapped_dx(from: u32, to: u32) -> i64 {
    let width = i64::from(ARENA_WIDTH);
    let raw = i64::from(to) - i64::from(from);
    [raw, raw - width, raw + width]
        .into_iter()
        .min_by_key(|delta| delta.abs())
        .expect("three wrapped displacements")
}

fn arena_distance_squared(a: Site, b: Site) -> i64 {
    let dx = wrapped_dx(a.x, b.x);
    let dz = i64::from(b.z) - i64::from(a.z);
    dx * dx + dz * dz
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ScheduleMode, SimConfig};

    use cubarium_voxel_fauna::{Actions, Controller, Response, Scripted};

    const SEEDS: [u64; 4] = [1, 2, 3, 95];
    const STAGE_A_SEEDS: [u64; 12] = [1, 2, 4, 5, 6, 8, 9, 10, 13, 14, 17, 18];
    const REACQUISITION_SEEDS: [u64; 4] = [1, 2, 17, 95];

    /// P5-B's arena: every kind is laid, the stripped crown holds stock only above the
    /// band, the fine arena is the standard layout at twice the resolution, and the
    /// bystanders are the other lineage, placed and alive.
    #[test]
    fn the_rebuilt_arena_lays_every_kind_on_both_grids_with_bystanders() {
        for founder in Founder::ALL {
            for seed in [1u64, 6, 20] {
                let standard = Arena::build(founder, seed);
                let fine = Arena::build_on(founder, seed, ArenaGrid::Fine);
                assert_eq!(standard.resource_kinds, fine.resource_kinds);
                for (s, f) in standard.resources.iter().zip(&fine.resources) {
                    assert_eq!((2 * s.x, 2 * s.z), (f.x, f.z), "{founder:?} seed {seed}");
                    assert_eq!(f.y, ArenaGrid::Fine.ground_y());
                }
                let c = fine.world.config();
                assert_eq!((c.width, c.height, c.depth, c.voxel_m), (64, 32, 24, 0.125));
                for arena in [&standard, &fine] {
                    for (&site, &kind) in arena.resources.iter().zip(&arena.resource_kinds) {
                        let stock = arena.patch_stock(site);
                        if kind == PatchKind::Stripped {
                            assert_eq!(stock, 0.0, "a stripped crown is not in the band");
                            let crown = arena.flora.view().stand_at(site).expect("a crown").foliage;
                            assert!(crown > 0.0, "but it is there to be seen");
                        } else {
                            assert!(stock > 0.0, "{founder:?} {kind:?} at {site:?}");
                        }
                    }
                    assert_eq!(arena.bystanders.len(), ARENA_BYSTANDERS);
                    assert_eq!(arena.bystander_ids.len(), ARENA_BYSTANDERS, "all placed");
                    for &id in &arena.bystander_ids {
                        let b = arena.fauna.view().animal(id).expect("a bystander");
                        assert_ne!(b.founder, Some(founder), "the other lineage");
                    }
                }
                let kinds: std::collections::BTreeSet<_> =
                    standard.resource_kinds.iter().map(|k| k.as_str()).collect();
                assert_eq!(kinds.len(), 3, "{founder:?}: Stage A lays every kind");
            }
        }
    }

    /// [`Arena::build`]'s resource sites are always unique, including the seam layout
    /// (seeds `% 4 == 3`), where the seam tile used to collide with an interior pick
    /// (seed 95 laid `(0,4,8)` twice).
    #[test]
    fn resource_sites_are_unique_across_seeds() {
        for seed in SEEDS {
            for founder in Founder::ALL {
                let arena = Arena::build(founder, seed);
                let mut sorted = arena.resources.clone();
                sorted.sort();
                let n = sorted.len();
                sorted.dedup();
                assert_eq!(
                    sorted.len(),
                    n,
                    "{founder:?} seed {seed}: a resource site was selected twice"
                );
                if seed % 4 == 3 {
                    assert!(
                        sorted.iter().any(|s| s.x == 0),
                        "{founder:?} seed {seed}: the seam layout did not keep a seam tile"
                    );
                }
            }
        }
    }

    #[test]
    fn the_arena_is_the_frozen_size_with_finite_food() {
        for seed in SEEDS {
            for founder in Founder::ALL {
                let arena = Arena::build(founder, seed);
                let c = arena.world.config();
                assert_eq!((c.width, c.height, c.depth, c.voxel_m), (32, 16, 12, 0.25));
                assert!(!arena.resources.is_empty(), "{founder:?} seed {seed}");
                assert!(
                    arena.resource_stock() > 0.0,
                    "{founder:?} seed {seed}: the layout is not empty"
                );
                assert!(!arena.fauna.births_enabled(), "arenas disable paid births");
                let animal = arena
                    .animal_id
                    .and_then(|id| arena.fauna.view().animal(id))
                    .expect("an idle founder is placed");
                assert_eq!(animal.founder, Some(founder));
                assert!(arena.resources.iter().all(|r| *r != animal.site));
                // P2-C: the arena's founder arrives hungry, so eating has somewhere to
                // go. Half its structure, no reserve, one body_max of headroom.
                let sc = cubarium_voxel_fauna::FounderPhysiology::frozen(founder).core;
                assert_eq!(animal.body, 0.5 * sc.body_max, "{founder:?} seed {seed}");
                assert_eq!(animal.reserve, 0.0, "{founder:?} seed {seed}");
                assert!(animal.body > sc.body_min, "and it can still live");
            }
        }
    }

    /// The P2-B start convention: the founder still begins off food and inside its
    /// signal, but the heading no longer points at the food. The blind founder's heading
    /// is uniform over the whole circle; the browser's is inside +/-90 degrees of the
    /// bearing, so the foliage is somewhere the three sectors cover but usually not
    /// ahead.
    #[test]
    fn stage_a_starts_off_food_inside_signal_without_being_aimed_at_it() {
        let mut blind_errors: Vec<f64> = Vec::new();
        let mut browser_errors: Vec<f64> = Vec::new();
        for seed in STAGE_A_SEEDS {
            for founder in Founder::ALL {
                let arena = Arena::build(founder, seed);
                let animal = arena
                    .animal_id
                    .and_then(|id| arena.fauna.view().animal(id))
                    .expect("the Stage A founder is placed");
                let (target, distance) = arena
                    .resources
                    .iter()
                    .copied()
                    .map(|r| (r, arena_distance_squared(animal.site, r)))
                    .min_by_key(|(_, distance)| *distance)
                    .expect("the arena has resources");
                assert!(
                    (4..=16).contains(&distance),
                    "{founder:?} seed {seed}: start {:?}, nearest {:?}, distance^2 {distance}",
                    animal.site,
                    target,
                );

                let toward = (wrapped_dx(animal.site.x, target.x) as f64)
                    .atan2(f64::from(target.z) - f64::from(animal.site.z));
                let error = (animal.pose.heading_rad - toward)
                    .sin()
                    .atan2((animal.pose.heading_rad - toward).cos());
                match founder {
                    Founder::Blind => blind_errors.push(error),
                    Founder::Browser => {
                        assert!(
                            error.abs() <= std::f64::consts::FRAC_PI_2 + 1e-12,
                            "seed {seed}: the browser's target left the sector fan at \
                             {} degrees",
                            error.to_degrees()
                        );
                        browser_errors.push(error);
                    }
                }
            }
        }
        // The blind founder is not aimed: over the twelve layouts the headings reach
        // well outside the old +/-5 degrees and both signs occur.
        let aimed = blind_errors
            .iter()
            .filter(|e| e.abs() <= 5.0_f64.to_radians())
            .count();
        assert!(
            aimed <= 1,
            "the blind start is still aimed at the food: {aimed} of {} within 5 degrees",
            blind_errors.len()
        );
        assert!(
            blind_errors.iter().any(|e| *e > 1.0) && blind_errors.iter().any(|e| *e < -1.0),
            "the blind heading should reach both sides of the circle: {blind_errors:?}"
        );
        // The browser is neither aimed nor pinned to one side of the fan.
        assert!(
            browser_errors
                .iter()
                .filter(|e| e.abs() <= 5.0_f64.to_radians())
                .count()
                <= 1,
            "the browser start is still aimed at the food: {browser_errors:?}"
        );
        assert!(
            browser_errors.iter().any(|e| *e > 0.3) && browser_errors.iter().any(|e| *e < -0.3),
            "the browser heading should vary across the fan: {browser_errors:?}"
        );
    }

    /// The heading is a pure function of the layout seed, like everything else the
    /// arena draws.
    #[test]
    fn the_start_heading_is_deterministic_per_seed() {
        for seed in STAGE_A_SEEDS {
            for founder in Founder::ALL {
                let a = Arena::build(founder, seed).animal_pose().expect("placed");
                let b = Arena::build(founder, seed).animal_pose().expect("placed");
                assert_eq!(a.heading_rad, b.heading_rad, "{founder:?} seed {seed}");
                assert!(a.heading_rad.is_finite());
            }
        }
    }

    /// Stage B is an explicit two-patch task, without changing Stage A's six-resource
    /// fixture. The sites are fixture metadata only; the founder remains off food and
    /// begins in the ordinary local signal range of its first finite patch.
    #[test]
    fn reacquisition_layouts_are_two_finite_deterministic_patches() {
        for seed in REACQUISITION_SEEDS {
            for founder in Founder::ALL {
                let mut stage = Arena::build_reacquisition(founder, seed);
                let again = Arena::build_reacquisition(founder, seed);
                assert_eq!(stage.arena.resources, again.arena.resources);
                assert_eq!(stage.initial_patch, again.initial_patch);
                assert_eq!(stage.successor_patch, again.successor_patch);
                assert_eq!(stage.arena.resources.len(), 2);
                assert_ne!(stage.initial_patch, stage.successor_patch);
                assert!(stage.arena.resources.contains(&stage.initial_patch));
                assert!(stage.arena.resources.contains(&stage.successor_patch));
                assert!(stage.arena.resource_stock() > 0.0);
                assert!(
                    patch_stock(&stage.arena, stage.initial_patch) > 0.0,
                    "{founder:?} seed {seed}: initial patch is not finite"
                );
                assert!(
                    patch_stock(&stage.arena, stage.successor_patch) > 0.0,
                    "{founder:?} seed {seed}: successor patch is not finite"
                );
                assert!(
                    arena_distance_squared(stage.initial_patch, stage.successor_patch) >= 64,
                    "{founder:?} seed {seed}: successor is not a separate reacquisition leg"
                );
                let animal = stage
                    .arena
                    .animal_id
                    .and_then(|id| stage.arena.fauna.view().animal(id))
                    .expect("a Stage B founder is placed");
                assert!(
                    (4..=16).contains(&arena_distance_squared(animal.site, stage.initial_patch)),
                    "{founder:?} seed {seed}: founder did not begin in its initial patch's signal"
                );
                let initial_stock = patch_stock(&stage.arena, stage.initial_patch);
                let successor_stock = patch_stock(&stage.arena, stage.successor_patch);
                let taken = stage
                    .arena
                    .take(stage.initial_patch, initial_stock)
                    .expect("each finite patch depletes through its founder's real source");
                assert!(taken.organic > 0.0, "{founder:?} seed {seed}");
                assert_eq!(patch_stock(&stage.arena, stage.initial_patch), 0.0);
                assert_eq!(
                    patch_stock(&stage.arena, stage.successor_patch),
                    successor_stock,
                    "{founder:?} seed {seed}: depletion crossed into the successor patch"
                );
            }
        }
    }

    /// Stage B's initial patch is half the successor: roughly one eighth and one quarter
    /// of the 2,400-tick horizon at full-effort contact. The crown stays the same size
    /// for the browser; only its edible foliage is halved.
    #[test]
    fn the_stage_b_initial_patch_is_half_the_successor() {
        for founder in Founder::ALL {
            let stage = Arena::build_reacquisition(founder, 6);
            let config = cubarium_voxel_fauna::FaunaConfig::default();
            let bite_per_s = config.founder(founder).core.bite_per_s;
            let initial = stage.arena.patch_stock(stage.initial_patch);
            let successor = stage.arena.patch_stock(stage.successor_patch);
            assert!((initial / successor - 0.5).abs() < 1e-12, "{founder:?}");
            assert!(
                (10.0..=20.0).contains(&(initial / bite_per_s)),
                "{founder:?}: the initial patch is not about one eighth of 120 s"
            );
            assert!(
                (20.0..=40.0).contains(&(successor / bite_per_s)),
                "{founder:?}: the successor is not about one quarter of 120 s"
            );
            // Both Stage-B patches' stock is in the band, or the task is impossible
            // (`tests/arena_band.rs` sweeps every layout, both grids and both ends of the
            // body sizes).
            // And Stage A's tiles are deliberately not that: they cannot deplete.
            let a = Arena::build(founder, 6);
            let tile = a.patch_stock(a.resources[0]);
            if founder == Founder::Blind {
                assert!(
                    tile / bite_per_s > 300.0,
                    "a Stage-A litter tile should outlast any horizon: {tile}"
                );
            }
        }
    }

    /// The seeded Stage B placement always has enough dry support for two separated
    /// patches and an in-signal founder start. This is a construction sweep, not a long
    /// simulation study.
    #[test]
    fn reacquisition_placement_is_safe_across_a_broad_seed_sample() {
        for seed in 0..256 {
            for founder in Founder::ALL {
                let stage = Arena::build_reacquisition(founder, seed);
                assert_eq!(stage.arena.resources.len(), 2, "{founder:?} seed {seed}");
                assert!(
                    patch_stock(&stage.arena, stage.initial_patch) > 0.0,
                    "{founder:?} seed {seed}: initial patch is not finite"
                );
                assert!(
                    patch_stock(&stage.arena, stage.successor_patch) > 0.0,
                    "{founder:?} seed {seed}: successor patch is not finite"
                );
                assert!(
                    stage.arena.animal_id.is_some(),
                    "{founder:?} seed {seed}: no in-signal founder start"
                );
            }
        }
    }

    /// The static arena steps the world's and the fauna's clocks, ages and maintains the
    /// body, and **holds the terrain, water and unrelated plants still**. No motion, no
    /// feeding. The plant check is the stands' actual state — species, site, wood and
    /// foliage — not a count, so an empty-to-less-empty stand shuffle cannot satisfy it.
    #[test]
    fn a_static_arena_advances_time_and_freezes_the_world() {
        let arena = Arena::build(Founder::Browser, 7);
        let before_material = arena.world.view().material.to_vec();
        let before_free = arena.world.view().free.to_vec();
        let before_stands: Vec<_> = arena
            .flora
            .view()
            .stands
            .iter()
            .map(|s| (s.site, s.species, s.wood, s.foliage))
            .collect();
        let before_stock = arena.resource_stock();
        let resources = arena.resources.clone();
        let before_animal = *arena.fauna.view().animal(arena.animal_id.unwrap()).unwrap();

        let mut sim = arena.into_sim(SimConfig::with_threads(1));
        assert_eq!(sim.mode(), ScheduleMode::Static);
        for _ in 0..10 {
            sim.step();
        }

        assert_eq!(sim.world().tick(), 10);
        assert_eq!(sim.fauna().tick(), 10);
        assert_eq!(
            sim.world().view().material,
            before_material,
            "terrain moved"
        );
        assert_eq!(sim.world().view().free, before_free, "water moved");
        let after_stands: Vec<_> = sim
            .flora()
            .view()
            .stands
            .iter()
            .map(|s| (s.site, s.species, s.wood, s.foliage))
            .collect();
        assert_eq!(after_stands, before_stands, "a stand grew, died or moved");

        let after = *sim
            .fauna()
            .view()
            .animal(before_animal.id)
            .expect("the idle body survives ten ticks");
        assert_eq!(after.age_ticks, before_animal.age_ticks + 10);
        assert!(
            after.organic() < before_animal.organic(),
            "maintenance was paid out of the body's own organic matter"
        );
        assert_eq!(after.site, before_animal.site, "the idle body moved");
        assert_eq!(after.pose, before_animal.pose, "the idle body's pose moved");
        assert_eq!(after.state, cubarium_voxel_fauna::State::Resting);

        let (_, flora, _) = sim.layers();
        let stock = match before_animal.founder {
            Some(Founder::Browser) => arena_stock(flora, Founder::Browser, &resources),
            _ => unreachable!("browser arena"),
        };
        assert_eq!(stock, before_stock, "the idle body ate nothing");
    }

    /// The layout's edible stock off a flora that has left the arena: what
    /// [`Arena::resource_stock`] reads, the band's stock on every resource site.
    fn arena_stock(flora: &Flora, founder: Founder, resources: &[Site]) -> f64 {
        let grid = ArenaGrid::Standard;
        resources
            .iter()
            .map(|&r| edible_stock(founder, flora, grid.voxel_m(), grid.ground_y(), r))
            .sum()
    }

    fn patch_stock(arena: &Arena, site: Site) -> f64 {
        arena.patch_stock(site)
    }

    fn module_channel(observation: &[f64], founder: Founder, module: &str, channel: &str) -> f64 {
        let manifest = founder.manifest();
        let module = manifest
            .modules
            .iter()
            .find(|candidate| candidate.name == module)
            .unwrap_or_else(|| panic!("{founder:?} manifest has no {module} module"));
        let channel = module
            .channels
            .iter()
            .position(|candidate| *candidate == channel)
            .unwrap_or_else(|| panic!("{} has no {channel} channel", module.name));
        observation[module.offset + channel]
    }

    /// The finite stock is real and earnable through the production withdrawals.
    #[test]
    fn the_finite_food_is_actually_takeable() {
        let mut arena = Arena::build(Founder::Blind, 1);
        let site = arena.resources[0];
        let before = arena.resource_stock();
        let taken = arena.take(site, 0.05).expect("litter off the tile");
        assert!(taken.organic > 0.0);
        assert!(arena.resource_stock() < before);

        let mut browser = Arena::build(Founder::Browser, 1);
        let site = browser.resources[0];
        let before = browser.resource_stock();
        let taken = browser.take(site, 0.01).expect("foliage off the stand");
        assert!(taken.organic > 0.0);
        assert!(browser.resource_stock() < before);
    }

    /// Depleting Stage B's first patch goes through the ordinary flora withdrawal. A new
    /// settled field then removes its contact/taste signal while the real successor
    /// remains chemically and mechanically present; no fixture-only target marker exists.
    #[test]
    fn reacquisition_depletion_updates_the_real_resource_and_sensory_sources() {
        let mut stage = Arena::build_reacquisition(Founder::Blind, 17);
        let initial = stage.initial_patch;
        let successor = stage.successor_patch;
        let before = stage.arena.resource_stock();
        let taken = stage
            .arena
            .take(initial, 1.0)
            .expect("the initial litter patch is finite and takeable");
        assert!(taken.organic > 0.0);
        assert!(stage.arena.resource_stock() < before);

        let prepared = stage.arena.prepare_senses();
        let initial_obs = first_sample_of_a_founder_on_the_tile(
            stage
                .arena
                .into_sim_prepared(SimConfig::with_threads(1), prepared),
            initial,
        );

        let stage = Arena::build_reacquisition(Founder::Blind, 17);
        let prepared = stage.arena.prepare_senses();
        let successor_obs = first_sample_of_a_founder_on_the_tile(
            stage
                .arena
                .into_sim_prepared(SimConfig::with_threads(1), prepared),
            successor,
        );
        assert_eq!(
            initial_obs.len(),
            Founder::Blind.manifest().inputs(),
            "the controller receives only its declared manifest vector"
        );
        assert_eq!(successor_obs.len(), initial_obs.len());
        assert_eq!(
            module_channel(&initial_obs, Founder::Blind, "Taste(1)", "cue"),
            0.0,
            "depleted litter has no mouth cue"
        );
        assert!(
            module_channel(&successor_obs, Founder::Blind, "Taste(1)", "cue") > 0.0,
            "the successor still has mouth cue"
        );
        assert_eq!(
            module_channel(&initial_obs, Founder::Blind, "Taste(1)", "valid"),
            1.0,
            "bare ground remains a valid taste contact"
        );
        let initial_chem =
            module_channel(&initial_obs, Founder::Blind, "Chem(detritus)", "response");
        let successor_chem =
            module_channel(&successor_obs, Founder::Blind, "Chem(detritus)", "response");
        assert!(
            successor_chem > initial_chem,
            "the surviving successor remains a stronger local cue: initial {}, successor {}",
            initial_chem,
            successor_chem,
        );
    }

    /// The live schedule is still the default; only an explicit static construction moves
    /// off it.
    #[test]
    fn live_is_still_the_default_mode() {
        let sim = Sim::from_configs(
            VoxelConfig::default(),
            FloraConfig::default(),
            FaunaConfig::default(),
            SimConfig::with_threads(1),
        );
        assert_eq!(sim.mode(), ScheduleMode::Live);
    }

    /// A founder driven through the production static schedule: the controller stage
    /// runs inside `sys_fauna`, the held actions move the body, and a held feed crops
    /// a stand through the same real withdrawals. No special arena path exists.
    #[test]
    fn a_controller_driven_founder_moves_and_feeds_through_the_static_schedule() {
        let arena = Arena::build(Founder::Browser, 11);
        let resources = arena.resources.clone();
        // The first stand whose stock the band reaches (a stripped crown is not food).
        let stand_site = arena
            .resources
            .iter()
            .copied()
            .find(|&r| arena.patch_stock(r) > 0.0)
            .expect("a reachable stand");
        let stock_before = arena.resource_stock();
        let idle_id = arena.animal_id.expect("the arena placed an idle body");

        // A founder body standing in the stand it will crop, off the arena's own idle
        // body (which has no controller and stays exactly where P1-A left it).
        let mut sim = arena.into_sim(SimConfig::with_threads(1));
        let (feeder, cruiser) = sim.with_layers_mut(|world, _, fauna| {
            let mut introduce = |z: u32| {
                assert!(fauna.apply(
                    world,
                    FaunaCommand::IntroduceFounder {
                        x: i64::from(stand_site.x),
                        z,
                        founder: Founder::Browser,
                        stores: cubarium_voxel_fauna::StartingStores {
                            body: 0.8,
                            reserve: 1.0,
                        },
                        heading_rad: 0.0,
                    },
                ));
                fauna.view().ledger.births - 1
            };
            // The feeder stands in the stand it will crop; the cruiser starts beside
            // it and just drives.
            let feeder = introduce(stand_site.z);
            let cruiser = introduce(stand_site.z + 3);
            (feeder, cruiser)
        });
        assert!(sim.fauna_mut().set_controller(
            feeder,
            Box::new(Scripted::new(vec![Actions {
                forward: 0.0,
                turn: 0.0,
                feed: 1.0,
            }])),
        ));
        assert!(sim.fauna_mut().set_controller(
            cruiser,
            Box::new(Scripted::new(vec![Actions {
                forward: 1.0,
                turn: 0.0,
                feed: 0.0,
            }])),
        ));

        // Five controller periods: five feed attempts for the stand's resident, and a
        // moved body for the cruiser.
        for _ in 0..25 {
            sim.step();
        }

        let a = sim
            .fauna()
            .view()
            .animal(feeder)
            .expect("the driven founder survived");
        assert_eq!(
            a.pose,
            cubarium_voxel_fauna::Pose::at_site(stand_site, ARENA_VOXEL_M),
            "a feeding founder stands still"
        );
        assert!(
            a.founder_state.feedback.attempted_equivalent == 0.0,
            "no motion was requested, so none was attempted"
        );
        assert!(
            sim.fauna().view().ledger.bites >= 4,
            "the held feed attempted once per interval: {} bites",
            sim.fauna().view().ledger.bites
        );
        assert!(
            sim.fauna().view().ledger.eaten_organic_in > 0.0,
            "the bites transferred real foliage"
        );
        let c = sim.fauna().view().animal(cruiser).unwrap();
        assert_ne!(
            c.pose,
            cubarium_voxel_fauna::Pose::at_site(
                Site {
                    x: stand_site.x,
                    y: stand_site.y,
                    z: stand_site.z + 3
                },
                ARENA_VOXEL_M
            ),
            "the cruiser's held forward action moved it"
        );
        // The browser layout's finite stock: every seeded stand's foliage, read through
        // the sim's flora — the same read `Arena::resource_stock` does.
        let stock_after = arena_stock(sim.flora(), Founder::Browser, &resources);
        assert!(
            stock_after < stock_before,
            "the finite stock went down through the production withdrawals"
        );

        // The idle founder body without a controller did not move.
        let idle = sim.fauna().view().animal(idle_id).unwrap();
        assert_eq!(idle.state, cubarium_voxel_fauna::State::Resting);
    }

    /// A controller that records every observation and rests: the probe for what the
    /// static tick's controller stage actually sampled.
    struct Recorder {
        log: std::sync::Arc<std::sync::Mutex<Vec<Vec<f64>>>>,
    }

    impl Recorder {
        fn new() -> (Recorder, std::sync::Arc<std::sync::Mutex<Vec<Vec<f64>>>>) {
            let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            (Recorder { log: log.clone() }, log)
        }
    }

    impl Controller for Recorder {
        fn drive(&mut self, observation: &[f64]) -> Response {
            self.log.lock().unwrap().push(observation.to_vec());
            Response::Bounded(cubarium_voxel_fauna::Actions::REST)
        }

        fn reset(&mut self) {
            self.log.lock().unwrap().clear();
        }
    }

    fn first_sample_of_built_founder(mut sim: Sim, id: u64) -> Vec<f64> {
        let (recorder, log) = Recorder::new();
        assert!(sim.fauna_mut().set_controller(id, Box::new(recorder)));
        for _ in 0..6 {
            sim.step();
        }
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 1, "one sampling in six ticks");
        log[0].clone()
    }

    /// Every sample a controller took, for a probe that runs a few seconds.
    fn samples_of_built_founder(mut sim: Sim, id: u64, ticks: u64, turn: f64) -> Vec<Vec<f64>> {
        let (recorder, log) = Recorder::new();
        assert!(sim.fauna_mut().set_controller(
            id,
            Box::new(Turning {
                turn,
                inner: recorder
            })
        ));
        for _ in 0..ticks {
            sim.step();
        }
        let log = log.lock().unwrap();
        log.clone()
    }

    /// A recorder that also holds a constant yaw: the probe for "could it have seen the
    /// food if it had looked?".
    struct Turning {
        turn: f64,
        inner: Recorder,
    }

    impl Controller for Turning {
        fn drive(&mut self, observation: &[f64]) -> Response {
            let _ = self.inner.drive(observation);
            Response::Bounded(Actions {
                forward: 0.0,
                turn: self.turn,
                feed: 0.0,
            })
        }

        fn reset(&mut self) {
            self.inner.reset();
        }
    }

    /// What the freed start heading (P2-B) still promises, and what it deliberately no
    /// longer does.
    ///
    /// The blind founder's cue is omnidirectional, so its promise is unchanged: a valid,
    /// nonzero `Chem(litter)` at the very first sampling on every layout.
    ///
    /// The browser's is now a *looking* promise. Its three sectors are a sparse fan
    /// (yaw -30/0/+30 within each of -60/0/+60, pitch -20/0/+20), and a half-grown
    /// springturf's crown is about 0.19 m across at under a metre, so a start that is no
    /// longer aimed at the stand can fall between rays: measured here, 11 of 12 layouts
    /// still hit foliage on the very first sample and seed 4 does not. The fan is
    /// therefore not the bottleneck the freed heading was meant to create — what the
    /// heading removes is "the food is straight ahead", not "the food is visible". What
    /// must hold either way is that the foliage is *there to be found*: a founder that
    /// simply turns on the spot sees it within one revolution (pi seconds at the
    /// 2 rad/s yaw cap) on every layout.
    ///
    /// **Since P5 (the 45-ray fan at pitches -40..+40 and the rebuilt arena) the promise
    /// is about edible foliage.** A Stage-A browser layout also carries stripped adults,
    /// and every rosette keeps its upper crown: foliage that is seen and cannot be eaten,
    /// a legitimate signal to learn about. With every layout as built, all 12 starts see
    /// *some* foliage at the first sample. So the "not aimed" half is measured on the
    /// layout with its inedible foliage removed (only what the adult band reaches left),
    /// and on the **centre** sector: the food is not straight ahead on every start. (It
    /// was "11 of 12 starts see edible foliage in any sector at once, never all" under
    /// the fixed sub-step march; the cell-exact traversal of cone-speed item 3 now
    /// catches a crown seed 17's left sector only clipped, so all 12 see some edible
    /// foliage somewhere — which the freed heading never promised to prevent — while 6
    /// of 12 centre sectors see none, as before.)
    /// The revolution half keeps the food **and the crowns standing over it** (a crown
    /// over a rosette is the honest mark of food) and removes only the stripped adults:
    /// every layout finds foliage within one turn on the spot. That needed the start to
    /// be aimed at edible patches only; aimed at the nearest site of any kind, seed 6's
    /// browser started beside a stripped crown and saw no food in a whole turn.
    #[test]
    fn stage_a_starts_present_the_promised_initial_signal() {
        let mut browser_immediate = 0usize;
        for seed in STAGE_A_SEEDS {
            for founder in Founder::ALL {
                let arena = Arena::build(founder, seed);
                let id = arena.animal_id.expect("the Stage A founder is placed");
                match founder {
                    Founder::Blind => {
                        let obs = first_sample_of_built_founder(
                            arena.into_sim(SimConfig::with_threads(1)),
                            id,
                        );
                        assert_eq!(obs[20], 1.0, "seed {seed}: chemical receptor valid");
                        assert!(obs[18] > 0.0, "seed {seed}: start is outside the cue");
                    }
                    Founder::Browser => {
                        // The first sample, with only edible foliage standing.
                        let mut edible = Arena::build(founder, seed);
                        strip_inedible_foliage(&mut edible, false);
                        let first = first_sample_of_built_founder(
                            edible.into_sim(SimConfig::with_threads(1)),
                            id,
                        );
                        // The revolution, with the food and the crowns over it.
                        let mut marked = arena;
                        strip_inedible_foliage(&mut marked, true);
                        // 80 ticks is 4 s: more than the pi seconds one revolution costs.
                        let samples = samples_of_built_founder(
                            marked.into_sim(SimConfig::with_threads(1)),
                            id,
                            80,
                            1.0,
                        );
                        assert!(samples.len() >= 8, "seed {seed}: the cone was not sampled");
                        let foliage = |o: &Vec<f64>| o[20] + o[26] + o[32];
                        assert!(
                            samples.iter().all(|o| o[36] == 1.0),
                            "seed {seed}: material cone invalid"
                        );
                        if first[26] > 0.0 {
                            browser_immediate += 1;
                        }
                        assert!(
                            samples.iter().any(|o| foliage(o) > 0.0),
                            "seed {seed}: one revolution on the spot never saw the foliage"
                        );
                    }
                }
            }
        }
        assert!(
            browser_immediate < STAGE_A_SEEDS.len(),
            "the browser start is still aimed: {browser_immediate} of {} layouts hit \
             edible foliage dead ahead (the centre sector) on the first sample",
            STAGE_A_SEEDS.len()
        );
    }

    /// Strip a browser arena's inedible foliage: the stripped adults go whole, and —
    /// unless `keep_marking_crowns` — every rosette's upper crown too, leaving exactly
    /// [`Arena::patch_stock`] standing.
    fn strip_inedible_foliage(arena: &mut Arena, keep_marking_crowns: bool) {
        let band = FaunaConfig::default()
            .founder(arena.founder)
            .adult_body()
            .mouth_layers(arena.grid.ground_y(), arena.grid.voxel_m());
        for (site, kind) in arena
            .resources
            .clone()
            .into_iter()
            .zip(arena.resource_kinds.clone())
        {
            if keep_marking_crowns && kind != PatchKind::Stripped {
                continue;
            }
            let above = (*band.end() + 1)..=i64::MAX;
            let foliage = arena.flora.view().stand_at(site).map_or(0.0, |s| s.foliage);
            if foliage > 0.0 {
                let _ = arena.flora.take_foliage_in_layers(site, foliage, &above);
            }
            let left = arena.flora.view().stand_at(site).map_or(0.0, |s| s.foliage);
            assert!(
                (left - arena.patch_stock(site)).abs() < 1e-12,
                "{site:?}: {left} left against an edible {}",
                arena.patch_stock(site)
            );
        }
    }

    /// Put a resting, recording founder on a litter tile of a static sim and return its
    /// first sampled observation.
    fn first_sample_of_a_founder_on_the_tile(mut sim: Sim, site: Site) -> Vec<f64> {
        let id = sim.with_layers_mut(|world, _, fauna| {
            assert!(fauna.apply(
                world,
                FaunaCommand::IntroduceFounder {
                    x: i64::from(site.x),
                    z: site.z,
                    founder: Founder::Blind,
                    stores: cubarium_voxel_fauna::StartingStores::FULL,
                    heading_rad: 0.0,
                },
            ));
            fauna.view().ledger.births - 1
        });
        let (recorder, log) = Recorder::new();
        assert!(sim.fauna_mut().set_controller(id, Box::new(recorder)));
        // Six ticks: the first sampling is at age 5, before the tick's own first field
        // update at tick 10 — whatever the controller read was settled in.
        for _ in 0..6 {
            sim.step();
        }
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 1, "one sampling in six ticks");
        log[0].clone()
    }

    /// The static tick samples the **settled field**: a founder standing on a litter tile
    /// reads a valid, nonzero `Chem(litter)` at its very first sampling. Taste separately
    /// reports the material at its current mouth contact. The live schedule, by contrast,
    /// has no field at all.
    #[test]
    fn the_static_tick_samples_the_settled_field() {
        let arena = Arena::build(Founder::Blind, 1);
        let site = arena.resources[0];
        let sim = arena.into_sim(SimConfig::with_threads(1));
        assert!(
            sim.senses().is_some(),
            "a static arena holds its settled field"
        );
        let obs = first_sample_of_a_founder_on_the_tile(sim, site);
        assert_eq!(obs.len(), 23);
        assert_eq!(obs[20], 1.0, "chem validity from the settled field");
        assert!(
            obs[18] > 0.0,
            "a cue stands on the source tile: {}",
            obs[18]
        );
        assert_eq!(obs[17], 1.0, "taste validity");
        assert!(obs[15] > 0.0, "litter is present at the mouth contact");
        assert_eq!(obs[21..23], [1.0, 1.0], "open-sky light, valid");

        // The live schedule is senses-free: no field resource, and its fauna path is
        // exactly what it was.
        let live = Sim::from_configs(
            VoxelConfig::default(),
            cubarium_voxel_flora::FloraConfig::default(),
            cubarium_voxel_fauna::FaunaConfig::default(),
            SimConfig::with_threads(1),
        );
        assert!(live.senses().is_none());
        assert_eq!(live.mode(), ScheduleMode::Live);
    }

    /// A prepared field is reused: a sim built from a caller-settled [`Senses`] reads the
    /// settled cue on the first sampling — a fresh field would still read zero there,
    /// because the tick's own first update is not due until tick 10. The episode-private
    /// trend stores are cleared on the way in, so the first sample's trend is zero.
    #[test]
    fn a_prepared_field_is_reused_on_the_first_sampling() {
        let arena = Arena::build(Founder::Blind, 1);
        let site = arena.resources[0];
        // Settle once through the same API the search fixture caches.
        let prepared = arena.prepare_senses();
        let sim = arena.into_sim_prepared(SimConfig::with_threads(1), prepared);
        let obs = first_sample_of_a_founder_on_the_tile(sim, site);
        assert_eq!(obs[20], 1.0, "chem validity");
        assert!(
            obs[18] > 0.0,
            "the prepared field was readable before any live update: {}",
            obs[18]
        );
        assert_eq!(obs[19], 0.0, "the first sample's trend is zero, not stale");
    }

    /// The setup and sensing numbers on the arena's own scale: build, settle (inside
    /// `new_static`), and the controller stage's sampling — one observation build with
    /// the cone — per step. Ignored because it is a named study, not CI work
    /// (`design/voxel-senses-phase1-tests.md` §3: measure setup separately from ticks);
    /// run it by name when the benchmark needs the current figures.
    #[test]
    #[ignore = "study: run by name for the setup and per-observation numbers"]
    fn the_setup_and_sampling_costs_are_measured() {
        use std::time::Instant;

        for founder in Founder::ALL {
            let t0 = Instant::now();
            let arena = Arena::build(founder, 1);
            let build = t0.elapsed();

            // An explicit settle, so the update count and the per-update cost are on
            // record alongside into_sim's own settle.
            let mut probe = cubarium_voxel_fauna::Senses::new();
            let t1 = Instant::now();
            let (updates, converged) = probe.settle(&arena.world.view(), &arena.flora.view());
            let settle = t1.elapsed();

            let t2 = Instant::now();
            let mut sim = arena.into_sim(SimConfig::with_threads(1));
            let into_sim = t2.elapsed();

            // One driven founder on an interior column: every controller period samples
            // one observation (blind: field + receptors; browser: 27-ray cone + receptors).
            // The idle body the builder placed samples too — two observers.
            let id = sim.with_layers_mut(|world, _, fauna| {
                assert!(world.view().surface_y(8, 4).is_some(), "an interior column");
                assert!(fauna.apply(
                    world,
                    FaunaCommand::IntroduceFounder {
                        x: 8,
                        z: 4,
                        founder,
                        stores: FOUNDER_START,
                        heading_rad: 0.0,
                    },
                ));
                fauna.view().ledger.births - 1
            });
            let (recorder, log) = Recorder::new();
            assert!(sim.fauna_mut().set_controller(id, Box::new(recorder)));

            let t3 = Instant::now();
            let steps = 100;
            for _ in 0..steps {
                sim.step();
            }
            let sampled = log.lock().unwrap().len() as f64;
            let tick_with_sampling = t3.elapsed() / steps;

            // The animal-free baseline, so the per-observation share is visible: remove
            // every body and time the same tick count.
            let sites: Vec<_> = sim
                .fauna()
                .view()
                .animals
                .iter()
                .map(|a| (i64::from(a.site.x), a.site.z))
                .collect();
            sim.with_layers_mut(|world, _, fauna| {
                for (x, z) in sites {
                    while fauna.apply(world, FaunaCommand::Remove { x, z }) {}
                }
            });
            let t4 = Instant::now();
            for _ in 0..steps {
                sim.step();
            }
            let tick_empty = t4.elapsed() / steps;
            println!(
                "{}: build {build:?}, settle {updates} updates in {settle:?} (converged \
                 {converged}), into_sim {into_sim:?}, tick+2 observers {tick_with_sampling:?}, \
                 tick empty {tick_empty:?}, per-observation ≈ {:?} ({} samplings of the driven \
                 body, the idle body samples too)",
                founder.name(),
                (tick_with_sampling
                    .checked_sub(tick_empty)
                    .unwrap_or_default()
                    * steps)
                    / (2 * sampled as u32),
                sampled,
            );
        }
    }
}
