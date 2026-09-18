//! The model-side support the **replacement-control study** needs: eager per-site mineral
//! provisioning (`Provision::AtCreation`), a seed bank that can be removed with `Clear`-style
//! bookings (`Command::ClearBank`), and the branching a matched arm is made of — a cloned
//! conditioned state that steps identically.
//!
//! Short function tests of those three boundaries and nothing else. Nothing here is a study:
//! there is no arm, no control and no coexistence claim in this file. The study itself is
//! `examples/replacement.rs`, whose own `#[cfg(test)] mod tests` carries the harness-level
//! cases (the cap arithmetic, the predeclared site list, the refusals).
//!
//! Conventions are `round5a.rs`/`round5b.rs`'s: `voxel_m` is 1 m, soil is wetted by adding
//! free water to an air cell and converting it, and the **world is never stepped**, so a
//! fixture's pore fraction is the condition it says it is and the only thing that moves
//! anything is the plant layer.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, Deposit, DepositKind, Flora, FloraConfig, Provision, Site, Species,
};

// ------------------------------------------------------------------- fixtures

/// `round5b.rs`'s `fill`: turn one air voxel into `material` holding exactly `pore` of that
/// material's own pore capacity, by adding the water first and converting after.
fn fill(w: &mut World, x: i64, y: u32, z: u32, material: Material, pore: f64) {
    let want = pore * material.pore_capacity() * w.config().voxel_volume();
    if want > 0.0 {
        let got = w.apply(WorldCommand::AddWater { x, y, z, volume_m3: want });
        assert!((got - want).abs() < 1e-12, "the void took {got} of {want}");
    }
    w.apply(WorldCommand::SetMaterial { x, y, z, material });
}

fn empty_world(width: u32, depth: u32) -> World {
    World::empty(VoxelConfig {
        width,
        height: 10,
        depth,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    })
}

/// `round5b.rs`'s strip: only the columns of `keep` are solid at all — bedrock at `y = 0`,
/// soil at `y = 1..=2` holding `pore` of soil's capacity — so every kept column's support
/// face is `y = 2` in open sky and every other column has **none**, which is how a fixture
/// pins a donor's dispersal candidates.
fn pillars(width: u32, keep: &[i64], pore: f64) -> World {
    let mut w = empty_world(width, 1);
    for x in 0..width as i64 {
        if keep.contains(&x) {
            for y in 1..=2 {
                fill(&mut w, x, y, 0, Material::Soil, pore);
            }
        } else {
            w.apply(WorldCommand::SetMaterial { x, y: 0, z: 0, material: Material::Air });
            assert!(
                cubarium_voxel_flora::highest_support(&w.view(), x, 0).is_none(),
                "({x},0) must be void"
            );
        }
    }
    w
}

/// A **roofed** strip: soil at `y = 1..=2` over the whole footprint with a rock ceiling at
/// `y = 4`, so every column has **two** support faces — the soil at `y = 2` and the rock at
/// `y = 4` — which is what makes it the fixture for "every support face, not every column".
fn roofed(width: u32, depth: u32) -> World {
    let mut w = empty_world(width, depth);
    for z in 0..depth {
        for x in 0..width as i64 {
            for y in 1..=2 {
                fill(&mut w, x, y, z, Material::Soil, 0.5);
            }
            w.apply(WorldCommand::SetMaterial { x, y: 4, z, material: Material::Rock });
        }
    }
    w
}

fn at(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

/// Every support face of the world, through the core's own predicate: what
/// `Provision::AtCreation` claims to provision, counted independently of it.
fn support_faces(world: &World) -> Vec<Site> {
    let view = world.view();
    let c = view.config;
    let mut out: Vec<Site> = Vec::new();
    for z in 0..c.depth {
        for x in 0..c.width as i64 {
            for y in view.supports_in_column(x, z) {
                out.push(Site { x: x as u32, y, z });
            }
        }
    }
    out.sort_unstable();
    out
}

/// The three residuals, as every round's tests compute them.
fn assert_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    let (o, n, e) = (
        v.organic() - v.ledger.expected_organic(),
        v.mineral() - v.ledger.expected_mineral(),
        v.energy() - v.ledger.expected_energy(),
    );
    assert!(o.abs() <= 1e-9 * v.organic().abs().max(1.0), "{when}: organic residual {o}");
    assert!(n.abs() <= 1e-9 * v.mineral().abs().max(1.0), "{when}: mineral residual {n}");
    assert!(e.abs() <= 1e-9 * v.energy().abs().max(1.0), "{when}: energy residual {e}");
}

/// A config in which one species funds a whole package in a single tick and hops exactly one
/// column — `round4.rs`'s `fast_donor` with the hop pinned, so a fixture can say **which**
/// site a package lands on.
fn fast_donor(config: &mut FloraConfig, species: Species) {
    let sc = config.species_mut(species);
    sc.propagule_rate = 3.0;
    sc.reserve_cap = 40.0;
    sc.hop = 1;
}

fn plant(flora: &mut Flora, world: &World, x: i64, species: Species) {
    let wood = flora.config().species(species).wood_max;
    assert!(
        flora.apply(world, Command::Seed { x, z: 0, species, wood }),
        "{} would not plant on column {x}",
        species.name()
    );
}

// --------------------------------------------------------- Provision::AtCreation

/// **Every support face, once, and the whole of it booked.** The roofed strip has two
/// support faces per column — the soil it stands on and the rock roof over it — so a
/// provisioning rule that walked columns instead of faces would provision half of them.
#[test]
fn at_creation_provisions_every_support_face_once_and_books_the_whole_inventory() {
    let world = roofed(4, 3);
    let faces = support_faces(&world);
    assert_eq!(faces.len(), 24, "4 x 3 columns with two faces each");

    let config = FloraConfig { provision: Provision::AtCreation, ..FloraConfig::default() };
    let mineral = config.initial_mineral;
    let flora = Flora::in_world(&world, config);
    let v = flora.view();

    let sites: Vec<Site> = v.ground.iter().map(|g| g.site).collect();
    assert_eq!(sites, faces, "one Ground per support face, in site order");
    assert!(v.ground.iter().all(|g| g.mineral == mineral), "each face holds initial_mineral");
    assert_eq!(v.ledger.seeded_mineral_in, faces.len() as f64 * mineral);
    assert_eq!(v.ledger.expected_mineral(), faces.len() as f64 * mineral);
    assert_eq!(v.mineral(), faces.len() as f64 * mineral, "the stock is the booking");
    assert_residuals(&flora, "at creation");

    // The default is untouched: the same world under `Lazy` starts with nothing at all.
    let lazy = Flora::in_world(&world, FloraConfig::default());
    assert!(lazy.view().ground.is_empty(), "Lazy provisions nothing at creation");
    assert_eq!(lazy.view().ledger.seeded_mineral_in, 0.0);
}

/// **The point of the option: matched fertility across arms.** One conditioned state, two
/// arms branched from it that spread differently — one keeps its donor, the other has it
/// cleared, which is the study's exclusion arm — and under `AtCreation` the mineral
/// **imported** into the two arms is the same number, while under `Lazy` the arm that
/// colonised more sites has imported more of it and any comparison between them is
/// confounded by its own colonisation.
///
/// The inventory an arm competes for is the **inflow** side of the ledger,
/// `seeded_mineral_in + deposited_mineral_in`, and that is what has to match.
/// `expected_mineral()` itself cannot match and must not: it is the inflow *minus* what the
/// arm booked out, and the exclusion arm books its resident out on purpose. So this test
/// pins both — the imports equal, and the net differing by exactly the removal the arm
/// declared.
#[test]
fn at_creation_keeps_the_imported_mineral_equal_across_arms_where_lazy_does_not() {
    /// One provisioning rule's numbers: what the conditioned state had imported, and then
    /// per arm the imports, the net inventory, the mineral booked out and the sites reached.
    struct Arms {
        imported_0: f64,
        imported: (f64, f64),
        net: (f64, f64),
        removed: (f64, f64),
        sites: (usize, usize),
    }
    let arms = |provision: Provision| {
        let world = pillars(4, &[0, 1, 2], 0.5);
        let mut config = FloraConfig { provision, ..FloraConfig::default() };
        fast_donor(&mut config, Species::Bloomcrown);
        // Two columns in reach, so an arm that keeps its donor can reach a site the other
        // one never does: that difference is the whole subject of this test.
        config.species_mut(Species::Bloomcrown).hop = 2;
        let mut flora = Flora::in_world(&world, config);
        plant(&mut flora, &world, 0, Species::Bloomcrown);
        // Conditioning: one tick, which funds and lands one package on column 1 or 2 — the
        // two support faces within the donor's hop, since column 3 is void.
        let mut conditioned_world = world.clone();
        flora.step(&mut conditioned_world);
        let imported = |f: &Flora| {
            f.view().ledger.seeded_mineral_in + f.view().ledger.deposited_mineral_in
        };
        let imported_0 = imported(&flora);

        // Arm A keeps the donor and goes on colonising; arm B loses it and its bank,
        // exactly as the study's exclusion arm loses its resident.
        let (mut a, mut wa) = (flora.clone(), conditioned_world.clone());
        let (mut b, mut wb) = (flora.clone(), conditioned_world.clone());
        assert!(b.apply(&wb, Command::Clear { x: 0, z: 0 }), "arm B clears its donor");
        let banks: usize = [1, 2]
            .into_iter()
            .filter(|&x| b.apply(&wb, Command::ClearBank { x, z: 0, species: Species::Bloomcrown }))
            .count();
        assert_eq!(banks, 1, "conditioning landed exactly one package, on column 1 or 2");
        for _ in 0..20 {
            a.step(&mut wa);
            b.step(&mut wb);
        }
        assert_residuals(&a, "arm A");
        assert_residuals(&b, "arm B");
        Arms {
            imported_0,
            imported: (imported(&a), imported(&b)),
            net: (a.view().ledger.expected_mineral(), b.view().ledger.expected_mineral()),
            removed: (a.view().ledger.removed_mineral_out, b.view().ledger.removed_mineral_out),
            sites: (a.view().ground.len(), b.view().ground.len()),
        }
    };

    let eager = arms(Provision::AtCreation);
    assert_eq!(
        eager.imported.0, eager.imported.1,
        "AtCreation: the two arms were given the same mineral inventory"
    );
    assert_eq!(
        eager.imported.0, eager.imported_0,
        "AtCreation: and neither imported any after the branch"
    );
    assert_eq!(eager.sites, (3, 3), "every support face was provisioned before the branch");
    assert_eq!(eager.removed.0, 0.0, "arm A books nothing out");
    assert!(eager.removed.1 > 0.0, "arm B books its resident out");
    let gap = (eager.net.0 - eager.net.1) - eager.removed.1;
    assert!(
        gap.abs() <= 1e-12 * eager.removed.1.max(1.0),
        "the net inventories must differ by exactly the removal arm B declared and by nothing \
         else: {} - {} against {}",
        eager.net.0,
        eager.net.1,
        eager.removed.1
    );

    let lazy = arms(Provision::Lazy);
    assert!(
        lazy.sites.0 > lazy.sites.1,
        "arm A reached {} sites, arm B {}: the arms must differ for this to matter",
        lazy.sites.0,
        lazy.sites.1
    );
    assert!(
        lazy.imported.0 > lazy.imported.1,
        "Lazy: arm A imported {} against arm B's {} — unmatched fertility",
        lazy.imported.0,
        lazy.imported.1
    );
    assert_eq!(
        lazy.imported.0 - lazy.imported.1,
        (lazy.sites.0 - lazy.sites.1) as f64 * FloraConfig::default().initial_mineral,
        "and the gap is exactly initial_mineral per extra site the arm reached"
    );
    assert!(lazy.imported.0 > lazy.imported_0, "Lazy imports while the plants are spreading");
}

// ------------------------------------------------------------ Command::ClearBank

/// A site holding **both** species' banks, plus litter, dead wood and its mineral pool: two
/// one-column-hop donors on columns 0 and 2 of a strip whose column 3 is void, so column 1
/// is the only face either of them can reach and both packages land on it.
fn two_banks_on_one_site() -> (World, Flora) {
    let world = pillars(4, &[0, 1, 2], 0.5);
    let mut config = FloraConfig::default();
    fast_donor(&mut config, Species::Bloomcrown);
    fast_donor(&mut config, Species::Umbrellafrond);
    let mut flora = Flora::new(config);
    plant(&mut flora, &world, 0, Species::Bloomcrown);
    plant(&mut flora, &world, 2, Species::Umbrellafrond);
    let mut stepped = world.clone();
    flora.step(&mut stepped);
    assert!(
        flora.deposit(at(1), Deposit { kind: DepositKind::Litter, organic: 0.2, mineral: 0.004, energy: 0.3 }),
        "the litter was refused"
    );
    assert!(
        flora.deposit(at(1), Deposit { kind: DepositKind::DeadWood, organic: 0.5, mineral: 0.01, energy: 1.0 }),
        "the log was refused"
    );
    let g = flora.view().ground_at(at(1)).expect("a bank site");
    assert!(g.seed_organic(Species::Bloomcrown) > 0.0, "no bloomcrown package reached (1,0)");
    assert!(g.seed_organic(Species::Umbrellafrond) > 0.0, "no umbrellafrond package reached (1,0)");
    (world, flora)
}

/// **One species out, everything else exactly as it was.** The exclusion arm's whole
/// requirement: the resident's bank is booked out like a `Clear`, and the water, the litter,
/// the dead wood, the soil mineral and the *other* species' cohorts are retained.
#[test]
fn clear_bank_books_one_species_out_and_leaves_the_rest_of_the_site_untouched() {
    let (world, mut flora) = two_banks_on_one_site();
    let before = flora.view().ground_at(at(1)).expect("a bank site").clone();
    let ledger = flora.view().ledger.clone();
    let e_v = flora.config().species(Species::Bloomcrown).energy_density;
    let (organic, mineral) =
        (before.seed_organic(Species::Bloomcrown), before.seed_mineral(Species::Bloomcrown));

    assert!(
        flora.apply(&world, Command::ClearBank { x: 1, z: 0, species: Species::Bloomcrown }),
        "the bloomcrown bank would not clear"
    );

    let after = flora.view().ground_at(at(1)).expect("the site is still there").clone();
    assert_eq!(after.seed_organic(Species::Bloomcrown), 0.0, "the resident's bank is gone");
    assert!(
        after.seeds.iter().all(|c| c.species != Species::Bloomcrown),
        "no bloomcrown cohort may survive: {:?}",
        after.seeds
    );
    // Retained, to the bit.
    assert_eq!(after.seed_organic(Species::Umbrellafrond), before.seed_organic(Species::Umbrellafrond));
    assert_eq!(after.seed_mineral(Species::Umbrellafrond), before.seed_mineral(Species::Umbrellafrond));
    assert_eq!(after.mineral, before.mineral, "the soil mineral pool is retained");
    assert_eq!(after.litter, before.litter, "the litter is retained");
    assert_eq!(after.litter_mineral, before.litter_mineral);
    assert_eq!(after.litter_energy, before.litter_energy);
    assert_eq!(after.dead_wood, before.dead_wood, "the dead wood is retained");
    assert_eq!(after.dead_wood_mineral, before.dead_wood_mineral);
    assert_eq!(after.dead_wood_energy, before.dead_wood_energy);

    // Booked out, not dropped: a `Clear`'s three lines on a bank's three currencies.
    let now = flora.view().ledger.clone();
    assert_eq!(now.removed_organic_out - ledger.removed_organic_out, organic);
    assert_eq!(now.removed_mineral_out - ledger.removed_mineral_out, mineral);
    assert_eq!(now.removed_energy_out - ledger.removed_energy_out, e_v * organic);
    assert_eq!(now.births, ledger.births, "a removal is not a birth");
    assert_residuals(&flora, "after the bank removal");

    // And the refusals, which book nothing at all.
    let quiet = flora.view().ledger.clone();
    assert!(
        !flora.apply(&world, Command::ClearBank { x: 1, z: 0, species: Species::Bloomcrown }),
        "a bank that is already gone cannot be removed again"
    );
    assert!(
        !flora.apply(&world, Command::ClearBank { x: 1, z: 0, species: Species::Springturf }),
        "a species with no cohort here has no bank to remove"
    );
    assert!(
        !flora.apply(&world, Command::ClearBank { x: 3, z: 0, species: Species::Bloomcrown }),
        "column 3 has no support face at all"
    );
    assert_eq!(*flora.view().ledger, quiet, "a refused removal books nothing");
}

/// The exclusion arm as the study performs it, on the fixture: **`Clear` then `ClearBank`**
/// for the resident, and the water and the ground stocks retained. A `Clear` alone leaves
/// the bank germinating, which is the reason the second command exists.
#[test]
fn clearing_a_resident_without_its_bank_leaves_it_able_to_come_back() {
    let (world, flora) = two_banks_on_one_site();
    let stocks = |f: &Flora| {
        let g = f.view().ground_at(at(1)).expect("a bank site");
        (g.mineral, g.litter, g.dead_wood)
    };

    // The resident is bloomcrown: cleared, with and without its bank.
    let mut kept = flora.clone();
    let mut w1 = world.clone();
    assert!(kept.apply(&w1, Command::Clear { x: 0, z: 0 }));
    let mut removed = kept.clone();
    assert!(removed.apply(&world, Command::ClearBank { x: 1, z: 0, species: Species::Bloomcrown }));
    let mut w2 = world.clone();

    assert_eq!(stocks(&kept), stocks(&removed), "the site's own stocks are matched");
    for _ in 0..40 {
        kept.step(&mut w1);
        removed.step(&mut w2);
    }
    let alive = |f: &Flora| f.view().stands.iter().filter(|s| s.species == Species::Bloomcrown).count();
    assert!(alive(&kept) > 0, "the retained bank germinated: that is what a Clear leaves behind");
    assert_eq!(alive(&removed), 0, "the removed bank cannot recruit");
    assert_residuals(&kept, "bank kept");
    assert_residuals(&removed, "bank removed");
}

// ------------------------------------------------------------------ branching

/// **An arm is a clone.** The study branches one conditioned state into three arms by
/// cloning the world and the plant layer, so a clone that does not step identically would
/// make every difference between arms unreadable. Ten ticks, and the whole state compared:
/// the world by its own `PartialEq`, the layer by its stands, its ground and its ledger.
#[test]
fn a_cloned_conditioned_state_steps_identically_for_ten_ticks() {
    let mut world = pillars(4, &[0, 1, 2], 0.5);
    let mut config = FloraConfig { provision: Provision::AtCreation, ..FloraConfig::default() };
    fast_donor(&mut config, Species::Bloomcrown);
    let mut flora = Flora::in_world(&world, config);
    plant(&mut flora, &world, 0, Species::Bloomcrown);
    for _ in 0..5 {
        flora.step(&mut world);
    }
    assert!(!flora.view().ground.is_empty(), "the conditioned state must hold something");

    let (mut w2, mut f2) = (world.clone(), flora.clone());
    for _ in 0..10 {
        flora.step(&mut world);
        f2.step(&mut w2);
    }

    assert!(world == w2, "the world diverged");
    let (a, b) = (flora.view(), f2.view());
    assert_eq!(a.tick, b.tick, "the tick diverged");
    assert_eq!(a.stands, b.stands, "the stands diverged");
    assert_eq!(a.ground, b.ground, "the ground diverged");
    assert_eq!(a.ledger, b.ledger, "the ledger diverged");
}
