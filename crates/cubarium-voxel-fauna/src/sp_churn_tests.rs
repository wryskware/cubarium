//! **SP-CHURN's laws** (2026-10-14; written before the fix, from the probe of seed 2 at
//! m163-m175 on the terrarium-wide world): a seedporter crossing a field of small latticevines
//! looks at a vine at each leafy face its mouth passes, and each look took one of a fixed few
//! memory slots, so the hosts it had checked were forgotten within seconds, not within its
//! memory (`check_memory_s`), and it walked back to check them again.
//!
//! - **a checked host stays remembered for the memory**, however many other hosts it looks at
//!   meanwhile; so the check's order keeps dropping it while it is remembered empty;
//! - **the memory still ends**: what it looked at longer ago than its memory is forgotten, so
//!   the memory holds only what it can recall.
//!
//! Every bound comes from the config; no shipped number, ratio or world hash is asserted.

use crate::larva::LarvaHost;
use crate::treecheck::{order, recall, remember};
use crate::{Command, Fauna, FaunaConfig, Founder, StartingStores};
use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

const SP: Founder = Founder::Seedporter;

fn world() -> World {
    let mut w = World::empty(VoxelConfig { width: 32, height: 16, depth: 16, voxel_m: 0.25, seed: 3, day: DayClock { start_hour: 12.0, ..DayClock::default() }, ..VoxelConfig::default() });
    for z in 0..16 {
        for x in 0..32 {
            for y in 1..=2 {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
            }
        }
    }
    w
}

/// A fauna with one seedporter, its id, and its memory in ticks.
fn one_body() -> (Fauna, u64, u64) {
    let w = world();
    let mut f = Fauna::new(FaunaConfig::default());
    f.set_births_enabled(false);
    assert!(f.apply(&w, Command::IntroduceFounder { x: 16, z: 8, founder: SP, stores: StartingStores { body: 1.0, reserve: 0.3 }, heading_rad: 0.0 }));
    let id = f.animals.last().expect("introduced").id;
    let g = f.config.founder(SP).glider.expect("a glider");
    assert!(g.check_memory_s > 1.0, "fixture: a memory of more than a second");
    (f, id, cubarium_voxel::ticks_at_least_one(g.check_memory_s))
}

/// **A checked host stays remembered for the memory, however many others it looks at.** A body
/// remembers a host empty, then looks at many other vines, one a tick, all within its memory:
/// it still recalls the first as empty, and the check's order still drops it.
#[test]
fn a_checked_host_stays_remembered_for_the_memory_however_many_others_it_looks_at() {
    let (mut f, id, memory) = one_body();
    let checked = LarvaHost::Stand(1);
    f.tick = 10;
    remember(&mut f, id, checked, false);
    // Many more looks than any small slot count, spread inside half the memory.
    let looks = 256u64.min(memory / 2).max(1);
    let step = (memory / 2 / looks).max(1);
    for k in 0..looks {
        f.tick += step;
        remember(&mut f, id, LarvaHost::Vine(k as _), false);
    }
    let now = f.tick;
    assert!(now - 10 < memory, "fixture: still within the memory");
    let got = recall(&f, id, checked, now, memory).expect("the checked host is still remembered");
    assert!(!got.held_food, "as empty");
    let cands = [(1.0, checked, ()), (2.0, LarvaHost::Stand(2), ())];
    let kept = order(&cands, None, |h| recall(&f, id, h, now, memory));
    assert!(kept.iter().all(|c| c.1 != checked), "the check does not walk back to it: {:?}", kept.iter().map(|c| c.1).collect::<Vec<_>>());
}

/// **The memory still ends.** What a body looked at longer ago than its memory is forgotten
/// once the memory is tidied (every 256 ticks); what it looked at within it is kept.
#[test]
fn what_a_body_looked_at_longer_ago_than_its_memory_is_forgotten() {
    let (mut f, id, memory) = one_body();
    let (old, recent) = (LarvaHost::Stand(1), LarvaHost::Stand(2));
    f.tick = 0;
    remember(&mut f, id, old, false);
    // The first tidy at least a memory after the old look, the recent one just before it.
    let tidy = memory.div_ceil(256).max(1) * 256 + 256;
    f.tick = tidy - 1;
    remember(&mut f, id, recent, true);
    f.tick = tidy;
    crate::treecheck::prune(&mut f);
    let seen = &f.tree_checks[&id].seen;
    assert!(seen.iter().all(|c| c.host != old), "the lapsed look is forgotten: {seen:?}");
    assert!(seen.iter().any(|c| c.host == recent), "the recent one is kept: {seen:?}");
}
