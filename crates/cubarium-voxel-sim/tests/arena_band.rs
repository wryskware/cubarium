//! P5-B item 3, the rebuilt arena: **Stage B's two patches are reachable by the mouth
//! band from the start face on every layout** — both founders, both grids (0.25 m and the
//! new 0.125 m variant), both successor bands, and at both ends of the body sizes D11
//! samples (a newborn at `body_min` and an adult at `body_max`).
//!
//! Row e of the brief is why: the band is `[0, 1.33·H]` over the standing surface, and
//! the old arena laid Stage-B crowns a voxel above the head, which no body can now reach.
//! "Reachable by the band" is the vertical test the live mouth applies
//! (`Body::mouth_layers`): a ground pool at the standing face, or a stand's foliage layer
//! whose cell the band overlaps, with stock in it. Walking there is the policy's job.

use cubarium_voxel_fauna::{Body, Founder};
use cubarium_voxel_flora::{Site, Trophic};
use cubarium_voxel_sim::{Arena, ArenaGrid, SuccessorBand};

/// What a mouth band standing at `standing_y` could take off `patch`: the ground pools
/// the founder eats at that face, and the in-band layers of any stand its diet accepts.
fn in_band_stock(
    arena: &Arena,
    founder: Founder,
    patch: Site,
    body: &Body,
    standing_y: u32,
) -> f64 {
    let fv = arena.flora.view();
    let voxel_m = arena.world.config().voxel_m;
    let band = body.mouth_layers(standing_y, voxel_m);
    let pools = match founder {
        Founder::Blind => fv
            .ground_at(Site {
                y: standing_y,
                ..patch
            })
            .map_or(0.0, |g| g.litter + g.carrion),
        Founder::Browser => 0.0,
    };
    let stands: f64 = fv
        .stand_at(patch)
        .filter(|s| {
            let trophic = fv.config.species(s.species).trophic;
            match founder {
                Founder::Blind => trophic == Trophic::Saprotroph,
                Founder::Browser => trophic != Trophic::Saprotroph,
            }
        })
        .map_or(0.0, |s| {
            fv.layers(s)
                .filter(|l| band.contains(&l.cell))
                .map(|l| l.stock)
                .sum()
        });
    pools + stands
}

#[test]
fn stage_b_patches_are_reachable_by_the_band_from_the_start_face_on_every_layout() {
    for grid in [ArenaGrid::Standard, ArenaGrid::Fine] {
        for founder in Founder::ALL {
            for band in [SuccessorBand::Near, SuccessorBand::Landed] {
                for seed in 0..32u64 {
                    let (arena, initial, successor) =
                        Arena::build_reacquisition_on(founder, seed, band, grid).into_parts();
                    let start = arena
                        .start_face()
                        .unwrap_or_else(|| panic!("{founder:?} {grid:?} seed {seed}: placed"));
                    let phys = *arena.fauna.config().founder(founder);
                    for body_organic in [phys.core.body_min, phys.core.body_max] {
                        let body = phys.body_at(body_organic);
                        for (name, patch) in [("initial", initial), ("successor", successor)] {
                            let stock = in_band_stock(&arena, founder, patch, &body, start.y);
                            assert!(
                                stock > 0.0,
                                "{founder:?} {grid:?} {band:?} seed {seed}: the {name} patch \
                                 at {patch:?} holds nothing a body of {body_organic} standing \
                                 at y {} reaches (band {:?})",
                                start.y,
                                body.mouth_layers(start.y, arena.world.config().voxel_m),
                            );
                        }
                    }
                }
            }
        }
    }
}
