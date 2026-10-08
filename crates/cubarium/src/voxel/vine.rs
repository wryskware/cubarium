//! Legacy presenter entrypoint for the shared pure latticevine rules.
pub use cubarium_session::view_state::vine::*;

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel::Config as WorldConfig;
    use cubarium_voxel_flora::{Face, FaceDir, FaceDraw, Site, SpurPhase};

    fn world() -> WorldConfig {
        WorldConfig {
            width: 16,
            height: 12,
            depth: 6,
            ..WorldConfig::default()
        }
    }

    fn draw(face: Face, leafiness: f64) -> FaceDraw {
        FaceDraw {
            face,
            owner: 1,
            leafiness,
            rooted: false,
            dormant: false,
            spur: SpurPhase::Bare,
        }
    }

    /// A face facing the camera: up is `+y` and right is `+x`, so a vertical run's middle
    /// face has up and down set and a row's end has only its inner side.
    #[test]
    fn the_mask_reads_up_right_down_left_in_the_faces_plane() {
        let w = world();
        let at = |x, y| Face::new(x, y, 3, FaceDir::NegZ);
        let covered = [at(5, 4), at(5, 5), at(5, 6), at(6, 5)];
        let is = |f: Face| covered.contains(&f);
        assert_eq!(mask(at(5, 5), &w, is), 0b0111, "up, right, down");
        assert_eq!(mask(at(5, 4), &w, is), 0b0001, "up only");
        assert_eq!(mask(at(6, 5), &w, is), 0b1000, "left only");
        // A face looking another way is not a neighbour.
        let other = [Face::new(5, 6, 3, FaceDir::PosX)];
        assert_eq!(mask(at(5, 5), &w, |f| other.contains(&f)), 0);
        // `+z` faces turn: the camera's right is their left.
        let back = |x| Face::new(x, 5, 3, FaceDir::PosZ);
        assert_eq!(mask(back(5), &w, |f| f == back(6)), 0b1000);
        // A `+x` face's right is `+z`.
        let side = |z| Face::new(5, 5, z, FaceDir::PosX);
        assert_eq!(mask(side(3), &w, |f| f == side(4)), 0b0010);
    }

    #[test]
    fn the_mask_wraps_in_x_and_stops_at_the_worlds_edges() {
        let w = world();
        let f = Face::new(15, 11, 0, FaceDir::NegZ);
        let right = Face::new(0, 11, 0, FaceDir::NegZ);
        assert_eq!(mask(f, &w, |n| n == right), 0b0010, "x wraps");
        assert_eq!(mask(f, &w, |_| true), 0b1110, "nothing above the top row");
    }

    #[test]
    fn density_follows_leafiness_and_a_dormant_vine_is_bare() {
        assert_eq!(density(1.0, false), Density::Full);
        assert_eq!(density(0.66, false), Density::Full);
        assert_eq!(density(0.5, false), Density::Thin);
        assert_eq!(density(0.2, false), Density::Thin);
        assert_eq!(density(0.19, false), Density::Bare);
        assert_eq!(density(f64::NAN, false), Density::Bare);
        assert_eq!(density(1.0, true), Density::Bare);
    }

    #[test]
    fn an_accent_shows_only_in_its_spur_phase() {
        assert_eq!(accent(SpurPhase::Bud), Accent::Bud);
        assert_eq!(accent(SpurPhase::Flower), Accent::Flower);
        assert_eq!(accent(SpurPhase::Fruit), Accent::Fruit);
        assert_eq!(accent(SpurPhase::Bare), Accent::None);
        assert_eq!(accent(SpurPhase::Spent), Accent::None);
    }

    #[test]
    fn tile_names_follow_the_file_interface() {
        assert_eq!(tile_name(TileSet::Plain, Density::Full, 0xb, None), "vine-full-b");
        assert_eq!(tile_name(TileSet::Plain, Density::Full, 0xb, Some(0xf)), "vine-full-b-b");
        assert_eq!(tile_name(TileSet::Climbing, Density::Bare, 0, Some(0)), "vine-root-bare-0-0");
        assert_eq!(
            tile_name(TileSet::Hanging, Density::Thin, 12, Some(4)),
            "vine-root-thin-c-4-hang"
        );
        assert_eq!(tile_column(0xb, 0xf), 0xb * 16 + 0xb);
        assert_eq!(accent_name(Accent::Flower), "vine-accent-flower");
    }

    /// Each face lands in the air voxel in front of it, and where two faces front one voxel
    /// the one facing the camera wins.
    #[test]
    fn cells_sit_in_front_and_the_most_visible_face_wins_a_shared_voxel() {
        let w = world();
        let front = Face::new(4, 2, 3, FaceDir::NegZ); // cell (4, 2, 2)
        let side = Face::new(3, 2, 2, FaceDir::PosX); // cell (4, 2, 2) too
        let under = Face::new(8, 5, 1, FaceDir::Down); // cell (8, 4, 1)
        let back = Face::new(10, 2, 1, FaceDir::PosZ);
        let mut d = vec![draw(side, 1.0), draw(front, 0.4), draw(under, 0.1), draw(back, 1.0)];
        d[1].spur = SpurPhase::Fruit;
        let cells = cells(&d, &w, |_| None);
        assert_eq!(cells.len(), 3);
        let b = cells.iter().find(|c| (c.x, c.y, c.z) == (10, 2, 2)).unwrap();
        assert!(!b.kind.tiled(), "a `+z` face is a plain cell only");
        let c = cells.iter().find(|c| (c.x, c.y, c.z) == (4, 2, 2)).unwrap();
        assert_eq!(c.kind, CellKind::Flush);
        assert_eq!((c.density, c.accent), (Density::Thin, Accent::Fruit));
        let u = cells.iter().find(|c| (c.x, c.y, c.z) == (8, 4, 1)).unwrap();
        assert_eq!((u.kind, u.density), (CellKind::Curtain, Density::Bare));
    }

    /// A rooted face on a ledge's lip hangs and draws the `-hang` tile; one at a wall's foot
    /// climbs.
    #[test]
    fn a_hanging_root_draws_the_hanging_tile() {
        let w = world();
        let root = Site { x: 4, y: 6, z: 3 };
        let lip = Face::new(4, 6, 3, FaceDir::NegZ);
        let below = Face::new(4, 5, 3, FaceDir::NegZ);
        let mut d = vec![draw(lip, 1.0), draw(below, 1.0)];
        d[0].rooted = true;
        let c = cells(&d, &w, |_| Some(root));
        let top = c.iter().find(|c| c.y == 6).unwrap();
        assert!(top.hanging);
        assert_eq!(top.mask, 0b0100, "covered below");
        assert_eq!(top.tile().0, TileSet::Hanging);
        assert_eq!(top.tile().2, 0b0100);
        let under = c.iter().find(|c| c.y == 5).unwrap();
        assert_eq!(under.tile().0, TileSet::Plain);
        let foot = Site { x: 4, y: 5, z: 3 };
        assert!(!hanging(Face::new(4, 6, 3, FaceDir::NegZ), foot), "the foot climbs");
        assert_eq!(flip_mask(0b1011), 0b1110);
    }

    #[test]
    fn the_texel_bytes_carry_every_field() {
        let c = VineCell {
            x: 0,
            y: 0,
            z: 0,
            kind: CellKind::SliverRight,
            rooted: true,
            hanging: true,
            density: Density::Thin,
            mask: 0b1001,
            exits: 0b1111,
            accent: Accent::Flower,
        };
        // Rooted and hanging, thin: row 3 · 2 + 1; exits only where the mask is set.
        assert_eq!(c.texel_bytes(), (4, 0b1001 | 0b1001 << 4, 7 | 2 << 4));
    }

    /// The default look: each covered face is a plain crown cell in the air in front of
    /// it, in the vine's colours; a face in flower is a heart cell tinted lilac with the
    /// cyan mouth as its glint; and a stand in the same cell keeps it.
    #[test]
    fn covered_faces_are_plain_voxel_cells_and_an_organism_keeps_its_cell() {
        use crate::voxel::stand::{Part, Stands};
        use cubarium_voxel::{Command, Material, World};
        use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species, VineSeed};
        let mut world = World::empty(world());
        for z in 0..6u32 {
            for x in 0..16i64 {
                for y in 0..3u32 {
                    let material = if y == 0 { Material::Bedrock } else { Material::Soil };
                    world.apply(Command::SetMaterial { x, y, z, material });
                }
                if (4..=10).contains(&x) && z >= 3 {
                    for y in 3..9u32 {
                        world.apply(Command::SetMaterial { x, y, z, material: Material::Rock });
                    }
                }
            }
        }
        let mut flora = Flora::new(FloraConfig::default());
        let face = |x, y| Face::new(x, y, 3, FaceDir::NegZ);
        let vine = flora
            .seed_vine(
                &world,
                VineSeed { root: Site { x: 6, y: 2, z: 2 }, first: face(6, 3), reserve: 1.0, lineage: None },
            )
            .unwrap();
        assert!(flora.cover_face(&world, vine, face(6, 4), 1.0));
        assert!(flora.cover_face(&world, vine, face(7, 4), 0.1));
        let view = world.view();
        let mut stands = Stands::empty(16, 12, 6);
        stands.rebuild(&view, flora.view());
        let full = crate::present::srgb_linear(crate::voxel::colours::vine(Density::Full, Accent::None).body);
        let part = stands.at(6, 4, 2);
        assert!(matches!(part, Part::Crown { heart: false, .. }), "{part:?}");
        assert_eq!(stands.style(part).unwrap().crown, full);
        let bare = crate::present::srgb_linear(crate::voxel::colours::vine(Density::Bare, Accent::None).body);
        assert_eq!(stands.style(stands.at(7, 4, 2)).unwrap().crown, bare);

        let mut draws = flora.view().cover.draw();
        for d in &mut draws {
            if d.face == face(6, 4) {
                d.spur = SpurPhase::Flower;
            }
        }
        stands.set_cover_draws(Some(draws));
        stands.rebuild(&view, flora.view());
        let part = stands.at(6, 4, 2);
        assert!(matches!(part, Part::Crown { heart: true, .. }), "{part:?}");
        let lilac = crate::present::srgb_linear(0xB9_9BE6);
        assert_eq!(stands.style(part).unwrap().crown, lilac);

        let wood = FloraConfig::default().species(Species::Bloomcrown).wood_max * 0.4;
        assert!(flora.apply(&world, FloraCommand::Seed { x: 6, z: 2, species: Species::Bloomcrown, wood }));
        stands.set_cover_draws(None);
        stands.rebuild(&view, flora.view());
        assert!(matches!(stands.at(6, 3, 2), Part::Trunk(_)), "the stand keeps its cell");
    }

    /// The faces on either side of a shared edge pick the same crossing, for every edge
    /// and every direction; and both crossings turn up across a wall.
    #[test]
    fn neighbours_agree_on_where_a_runner_crosses_their_shared_edge() {
        let w = world();
        let mut seen = [false; 2];
        for dir in FaceDir::ALL {
            for y in 1..10u32 {
                for x in 0..16u32 {
                    let f = Face::new(x, y, 2, dir);
                    let steps = plane_steps(dir);
                    for (bit, (dx, dy, dz)) in steps.into_iter().enumerate() {
                        let n = Face::new(
                            (i64::from(x) + dx).rem_euclid(16) as u32,
                            (i64::from(y) + dy) as u32,
                            (2 + dz) as u32,
                            dir,
                        );
                        let back = (bit + 2) % 4;
                        let mine = exits(f, 1 << bit, &w) >> bit & 1;
                        let theirs = exits(n, 1 << back, &w) >> back & 1;
                        assert_eq!(mine, theirs, "{f:?} and {n:?}");
                        seen[usize::from(mine)] = true;
                    }
                }
            }
        }
        assert_eq!(seen, [true, true]);
        assert_eq!(exits(Face::new(3, 3, 2, FaceDir::NegZ), 0, &w), 0, "no edge, no exit");
    }
}
