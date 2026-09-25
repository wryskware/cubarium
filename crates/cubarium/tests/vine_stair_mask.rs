//! **The latticevine tile mask across a stair**: a runner crossing a one-voxel tread joins
//! two side faces with the same direction, one voxel up and one voxel back. The lower face's
//! tile must reach up (mask bit 0) and the upper face's down (bit 2), and both must put the
//! runner at the same crossing (`exits`), or the drawn runner breaks at every step.

use cubarium::voxel::vine::{exits, mask};
use cubarium_voxel::Config as WorldConfig;
use cubarium_voxel_flora::{Face, FaceDir};

const UP: u8 = 1 << 0;
const DOWN: u8 = 1 << 2;

fn world() -> WorldConfig {
    WorldConfig {
        width: 16,
        height: 12,
        depth: 6,
        ..WorldConfig::default()
    }
}

/// The face one step up a staircase from `f`: one voxel up and one voxel back against its
/// outward offset, the same direction. Written out here, not taken from the crate.
fn stair_up(f: Face, w: &WorldConfig) -> Face {
    let (ox, _, oz) = f.dir.offset();
    Face::new(
        (i64::from(f.x) - ox).rem_euclid(i64::from(w.width)) as u32,
        f.y + 1,
        (i64::from(f.z) - oz) as u32,
        f.dir,
    )
}

/// The same face one voxel up in its own plane.
fn planar_up(f: Face) -> Face {
    Face::new(f.x, f.y + 1, f.z, f.dir)
}

#[test]
fn a_covered_stair_pair_links_the_lower_faces_up_to_the_upper_faces_down() {
    let w = world();
    for dir in FaceDir::SIDES {
        let lower = Face::new(7, 4, 2, dir);
        let upper = stair_up(lower, &w);
        let pair = |f: Face| f == lower || f == upper;
        assert_eq!(
            mask(lower, &w, pair),
            UP,
            "{dir:?}: the lower riser reaches up the stair to {upper:?}"
        );
        assert_eq!(
            mask(upper, &w, pair),
            DOWN,
            "{dir:?}: the upper riser reaches down the stair to {lower:?}"
        );
    }
}

#[test]
fn an_uncovered_stair_partner_leaves_the_bit_clear() {
    let w = world();
    for dir in FaceDir::SIDES {
        let lower = Face::new(7, 4, 2, dir);
        let upper = stair_up(lower, &w);
        assert_eq!(mask(lower, &w, |f| f == lower), 0, "{dir:?}: alone");
        assert_eq!(mask(upper, &w, |f| f == upper), 0, "{dir:?}: alone");
        // A stair partner looking another way is no partner.
        let turned = Face::new(upper.x, upper.y, upper.z, FaceDir::Down);
        assert_eq!(mask(lower, &w, |f| f == turned), 0, "{dir:?}: turned");
    }
}

#[test]
fn a_covered_planar_neighbour_still_sets_the_up_and_down_bits() {
    let w = world();
    for dir in FaceDir::SIDES {
        let lower = Face::new(7, 4, 2, dir);
        let above = planar_up(lower);
        let both = |f: Face| f == lower || f == above;
        assert_eq!(mask(lower, &w, both), UP, "{dir:?}: planar up");
        assert_eq!(mask(above, &w, both), DOWN, "{dir:?}: planar down");
        // Planar and stair both covered: still just the one bit.
        let stair = stair_up(lower, &w);
        let all = |f: Face| f == above || f == stair;
        assert_eq!(mask(lower, &w, all), UP, "{dir:?}: planar and stair up");
    }
}

/// An underside has no stair neighbours: with every face but its four planar neighbours
/// covered, its mask is empty. A side face with only its two stair partners covered has
/// exactly its up and down bits, never right or left.
#[test]
fn stairs_join_only_up_and_down_a_side_and_never_an_underside() {
    let w = world();
    let under = Face::new(7, 5, 2, FaceDir::Down);
    let planar = [
        Face::new(8, 5, 2, FaceDir::Down),
        Face::new(6, 5, 2, FaceDir::Down),
        Face::new(7, 5, 3, FaceDir::Down),
        Face::new(7, 5, 1, FaceDir::Down),
    ];
    assert_eq!(mask(under, &w, |f| !planar.contains(&f)), 0);
    for dir in FaceDir::SIDES {
        let mid = Face::new(7, 5, 2, dir);
        let up = stair_up(mid, &w);
        let (ox, _, oz) = dir.offset();
        let down = Face::new((7 + ox) as u32, 4, (2 + oz) as u32, dir);
        assert_eq!(stair_up(down, &w), mid, "fixture");
        assert_eq!(
            mask(mid, &w, |f| f == up || f == down),
            UP | DOWN,
            "{dir:?}: both stair partners"
        );
    }
}

/// x wraps with the ring; z does not: a stair step off the world's z walls is uncovered
/// even when the wrapped face is.
#[test]
fn a_stair_wraps_across_the_x_seam_and_stops_at_the_z_walls() {
    let w = world();
    let seam = Face::new(15, 4, 2, FaceDir::NegX);
    let over = Face::new(0, 5, 2, FaceDir::NegX);
    assert_eq!(mask(seam, &w, |f| f == over), UP, "x wraps");
    assert_eq!(mask(over, &w, |f| f == seam), DOWN, "x wraps back");
    // NegZ climbs +z: from the far wall the step up leaves the world.
    let far = Face::new(7, 4, w.depth - 1, FaceDir::NegZ);
    let wrapped = Face::new(7, 5, 0, FaceDir::NegZ);
    assert_eq!(
        mask(far, &w, |f| f == wrapped),
        0,
        "no wrap in z at the far wall"
    );
    // PosZ climbs −z: from the near wall the step up leaves the world.
    let near = Face::new(7, 4, 0, FaceDir::PosZ);
    let wrapped = Face::new(7, 5, w.depth - 1, FaceDir::PosZ);
    assert_eq!(
        mask(near, &w, |f| f == wrapped),
        0,
        "no wrap in z at the near wall"
    );
}

/// The two faces of a stair pair pick the same crossing for the runner between them, for
/// every side direction and every position (the x seam included), and both crossings turn
/// up somewhere.
#[test]
fn both_faces_of_a_stair_pair_agree_on_the_runners_crossing() {
    let w = world();
    let mut seen = [false; 2];
    for dir in FaceDir::SIDES {
        for y in 1..10u32 {
            for x in 0..16u32 {
                for z in 1..5u32 {
                    let lower = Face::new(x, y, z, dir);
                    let upper = stair_up(lower, &w);
                    let pair = |f: Face| f == lower || f == upper;
                    let (ml, mu) = (mask(lower, &w, pair), mask(upper, &w, pair));
                    assert_eq!((ml, mu), (UP, DOWN), "{lower:?} / {upper:?}");
                    let mine = exits(lower, ml, &w) & 1;
                    let theirs = (exits(upper, mu, &w) >> 2) & 1;
                    assert_eq!(mine, theirs, "{lower:?} up-exit vs {upper:?} down-exit");
                    seen[usize::from(mine)] = true;
                }
            }
        }
    }
    assert_eq!(
        seen,
        [true, true],
        "both crossings turn up across the stairs"
    );
}
