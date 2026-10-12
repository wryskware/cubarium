//! KIN-TRAIN's mechanism checks, on hand-built records and small worlds. No tuned number
//! is asserted: shares and scores here are fixture values.

use super::roster::{KinRecord, Relation, Roster};

fn rec(id: u64, mother: Option<u64>, pack: Option<u64>) -> KinRecord {
    KinRecord { id, mother, pack }
}

fn rel(r: &Roster, a: u64, b: u64) -> Option<Relation> {
    r.relatives(a).iter().find(|(o, _)| *o == b).map(|(_, x)| *x)
}

#[test]
fn roster_reads_mother_calf_sibling_and_packmate() {
    // 1 is the mother of 2 and 3; 4 is a stranger; 5 and 6 share pack 9 with 1.
    let r = Roster::fixed(&[
        rec(1, None, Some(9)),
        rec(2, Some(1), None),
        rec(3, Some(1), None),
        rec(4, None, None),
        rec(5, None, Some(9)),
        rec(6, None, Some(9)),
    ]);
    assert_eq!(rel(&r, 1, 2), Some(Relation::Calf));
    assert_eq!(rel(&r, 2, 1), Some(Relation::Mother));
    assert_eq!(rel(&r, 2, 3), Some(Relation::Sibling));
    assert_eq!(rel(&r, 3, 2), Some(Relation::Sibling));
    assert_eq!(rel(&r, 5, 6), Some(Relation::Packmate));
    assert_eq!(rel(&r, 1, 5), Some(Relation::Packmate));
    assert!(r.relatives(4).is_empty(), "a stranger has no relatives");
    assert_eq!(rel(&r, 1, 1), None, "never its own relative");
}

#[test]
fn the_closest_relation_wins_one_per_pair() {
    // A pup in its mother's pack is her calf, not her packmate; two littermates in one
    // pack are siblings, not packmates.
    let r = Roster::fixed(&[
        rec(1, None, Some(7)),
        rec(2, Some(1), Some(7)),
        rec(3, Some(1), Some(7)),
        rec(4, None, Some(7)),
    ]);
    for (a, b) in [(1, 2), (2, 1), (2, 3), (1, 4)] {
        let n = r.relatives(a).iter().filter(|(o, _)| *o == b).count();
        assert_eq!(n, 1, "{a}→{b} holds one relation");
    }
    assert_eq!(rel(&r, 1, 2), Some(Relation::Calf));
    assert_eq!(rel(&r, 2, 1), Some(Relation::Mother));
    assert_eq!(rel(&r, 2, 3), Some(Relation::Sibling));
    assert_eq!(rel(&r, 2, 4), Some(Relation::Packmate));
}

#[test]
fn a_founder_with_a_phantom_mother_id_has_no_live_mother() {
    // Founders 2 and 3 carry founding mother id 100, which names no animal.
    let r = Roster::fixed(&[rec(2, Some(100), None), rec(3, Some(100), None)]);
    assert!(!r.contains(100));
    assert!(
        r.relatives(2).iter().all(|(_, x)| *x != Relation::Mother),
        "no mother relation to an id that names no body"
    );
    assert_eq!(rel(&r, 2, 3), Some(Relation::Sibling), "a shared recorded id is a sibling key");
}

#[test]
fn a_newborn_joins_its_mothers_roster_at_birth() {
    let mut r = Roster::fixed(&[rec(1, None, Some(5)), rec(2, Some(1), Some(5)), rec(3, None, Some(5))]);
    r.join(rec(10, Some(1), Some(5)));
    assert_eq!(rel(&r, 1, 10), Some(Relation::Calf));
    assert_eq!(rel(&r, 10, 1), Some(Relation::Mother));
    assert_eq!(rel(&r, 2, 10), Some(Relation::Sibling));
    assert_eq!(rel(&r, 3, 10), Some(Relation::Packmate));
    // Joining twice changes nothing: relations are fixed once made.
    let before = r.relatives(1).len();
    r.join(rec(10, None, None));
    assert_eq!(r.relatives(1).len(), before);
}

#[test]
fn families_are_the_connected_groups() {
    let r = Roster::fixed(&[
        rec(1, None, None),
        rec(2, Some(1), None),
        rec(3, None, Some(4)),
        rec(5, None, Some(4)),
        rec(6, None, None),
    ]);
    assert_eq!(r.families(), vec![vec![1, 2], vec![3, 5], vec![6]]);
}

/// A small authored strip with `counts` founders, on their heuristics, one thread.
pub(super) fn small_sim(counts: &[(cubarium_voxel_fauna::Founder, usize)]) -> cubarium_voxel_sim::Sim {
    use cubarium_voxel_fauna::Founder;
    let cfg = cubarium_voxel::Config { width: 64, height: 24, depth: 6, ..Default::default() };
    let mut world = cubarium_voxel_sim::scene::authored(cfg);
    let mut flora = cubarium_voxel_flora::Flora::new(Default::default());
    let mut fauna =
        cubarium_voxel_fauna::Fauna::new(cubarium_voxel_fauna::FaunaConfig::placeholder());
    let mut n = [0; Founder::COUNT];
    for (f, k) in counts {
        n[f.index()] = *k;
    }
    cubarium_voxel_sim::habitat::seed_with_founder_counts(&mut world, &mut flora, &mut fauna, n);
    cubarium_voxel_sim::Sim::ambient(
        world,
        flora,
        fauna,
        cubarium_voxel_sim::SimConfig { threads: 1, overlap: false, water_every: 1 },
    )
}

#[test]
fn heuristic_bodies_are_metered_and_on_the_roster() {
    use crate::es::voxel::live::window::Window;
    use cubarium_voxel_fauna::Founder;
    let mut sim = small_sim(&[(Founder::Blind, 3)]);
    let ids: Vec<u64> = sim.fauna().view().animals.iter().map(|a| a.id).collect();
    assert!(!ids.is_empty(), "the fixture seeded bodies");
    // No lineage has a table: every body runs its heuristic.
    let tables = vec![None; Founder::COUNT];
    let mut w = Window::begin(&mut sim, &tables, &[0; Founder::COUNT], 40);
    while !w.done() {
        w.step(&mut sim);
    }
    let r = w.finish(&sim, 0);
    assert!(r.scores.is_empty(), "no variant, nothing ranked");
    for id in &ids {
        let m = r.metered.iter().find(|m| m.body_id == *id).expect("metered");
        assert_eq!(m.candidate, None);
        assert!(m.base().is_finite());
        assert!(r.roster.contains(*id), "on the roster");
    }
}
