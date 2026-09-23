//! The seed bank and dispersal (package S, `design/handoffs/voxel-plant-viability-2026-09-23.md`).
//! Tests first: the implementation follows in the next commit.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::{DOMAIN_WIND, Rng, establishment_gates_on_substrate};
    use crate::{Dispersal, Flora, FloraConfig, Ground, SeedCohort, Site, Species, Stage, Stand};
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

    const HOUR_TICKS: u64 = 3600 * cubarium_voxel::TICK_HZ as u64;

    /// `width × depth` columns of 1 m voxels: bedrock at `y = 0`, soil at `y = 1..=2`
    /// holding `pore` of its capacity, air above, no rain. Every column's support face is
    /// `y = 2`, in open sky.
    fn slab(width: u32, depth: u32, pore: f64) -> World {
        let config = VoxelConfig {
            width,
            height: 8,
            depth,
            voxel_m: 1.0,
            seed: 5,
            rain_m_per_s: 0.0,
            ..VoxelConfig::default()
        };
        let mut w = World::empty(config);
        for z in 0..depth {
            for x in 0..width as i64 {
                for y in 1..=2u32 {
                    soil(&mut w, x, y, z, pore);
                }
            }
        }
        w
    }

    /// One air voxel turned to soil holding `pore` of its capacity: water in first, then
    /// the conversion, so nothing is displaced.
    fn soil(w: &mut World, x: i64, y: u32, z: u32, pore: f64) {
        let want = pore * Material::Soil.pore_capacity() * w.config().voxel_volume();
        if want > 0.0 {
            w.apply(WorldCommand::AddWater {
                x,
                y,
                z,
                volume_m3: want,
            });
        }
        w.apply(WorldCommand::SetMaterial {
            x,
            y,
            z,
            material: Material::Soil,
        });
    }

    /// Raise one column's two soil voxels to `pore`: the gate opening. Each voxel is
    /// turned to air (its pore water becomes free water in the cell), topped up, and
    /// turned back.
    fn wet(w: &mut World, x: i64, z: u32, pore: f64) {
        let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
        for y in 1..=2u32 {
            let add = (pore - w.view().pore_at(x, y, z)) * cap;
            assert!(add >= 0.0, "wet only raises the water");
            w.apply(WorldCommand::SetMaterial {
                x,
                y,
                z,
                material: Material::Air,
            });
            w.apply(WorldCommand::AddWater {
                x,
                y,
                z,
                volume_m3: add,
            });
            w.apply(WorldCommand::SetMaterial {
                x,
                y,
                z,
                material: Material::Soil,
            });
        }
        assert!((w.view().pore_at(x, 2, z) - pore).abs() < 1e-9);
    }

    fn residuals(flora: &Flora) -> (f64, f64, f64) {
        let v = flora.view();
        (
            v.organic() - v.ledger.expected_organic(),
            v.mineral() - v.ledger.expected_mineral(),
            v.energy() - v.ledger.expected_energy(),
        )
    }

    fn assert_conserved(flora: &Flora, when: &str) {
        let v = flora.view();
        let (o, n, e) = residuals(flora);
        assert!(o.abs() <= 1e-9 * v.organic().max(1.0), "{when}: organic {o}");
        assert!(n.abs() <= 1e-9 * v.mineral().max(1.0), "{when}: mineral {n}");
        assert!(e.abs() <= 1e-9 * v.energy().max(1.0), "{when}: energy {e}");
    }

    /// `n` whole seeds of `species` banked on `site` in one bin opening now, booked as
    /// seeded material the way a founder is, and put on the check wheel.
    fn inject(flora: &mut Flora, site: Site, species: Species, n: u64) {
        let sc = flora.config.species(species).clone();
        let organic = n as f64 * package_of(&sc);
        let mineral = sc.n_tissue * organic;
        let gi = match flora.ground.binary_search_by_key(&site, |g| g.site) {
            Ok(i) => i,
            Err(i) => {
                flora.ground.insert(i, Ground::new(site, 1.0));
                flora.ledger.seeded_mineral_in += 1.0;
                i
            }
        };
        let g = &mut flora.ground[gi];
        g.seeds.push(SeedCohort {
            species,
            organic,
            mineral,
            bin_start_tick: flora.tick,
        });
        g.seeds.sort_by_key(|c| (c.species, c.bin_start_tick));
        flora.ledger.seeded_organic_in += organic;
        flora.ledger.seeded_mineral_in += mineral;
        flora.ledger.seeded_energy_in += sc.energy_density * organic;
        schedule(flora, site);
    }

    /// The first tick after `after` on which `site` is checked.
    fn next_check(site: Site, after: u64) -> u64 {
        let phase = check_phase(site);
        let t = after - after % CHECK_PERIOD_TICKS + phase;
        if t > after { t } else { t + CHECK_PERIOD_TICKS }
    }

    /// Step the plant layer (not the world) until its clock reads `tick`.
    fn run_to(flora: &mut Flora, world: &mut World, tick: u64) {
        assert!(flora.tick <= tick);
        while flora.tick < tick {
            flora.step(world);
        }
    }

    fn whole_seeds(flora: &Flora, site: Site, species: Species) -> f64 {
        let package = package_of(flora.config.species(species));
        flora
            .view()
            .ground_at(site)
            .map_or(0.0, |g| g.seed_organic(species) / package)
    }

    fn stand_on(site: Site, species: Species, wood: f64) -> Stand {
        Stand {
            id: 0,
            site,
            species,
            stage: Stage::Alive,
            wood,
            foliage: 0.0,
            reserve: 0.0,
            light: 0.0,
            moisture: 0.0,
            water_m3: 0.0,
            mineral: 0.0,
            aeration_stress: 0.0,
            parcel: 0.0,
            layer_stock: [0.0; crate::MAX_FOLIAGE_LAYERS],
            profile_stage: 0,
        }
    }

    /// Test 1. A lone seed on a site whose gates fail **waits**: it is refused at its
    /// check, still there hours later, and germinates at the first check after the gate
    /// opens — not before it, and not on the tick the soil was wetted.
    #[test]
    fn a_lone_seed_waits_hours_and_germinates_at_the_first_check_after_the_gate_opens() {
        let mut config = FloraConfig::default();
        // This test is about waiting, not about attrition.
        config.bloomcrown.seed_attrition_per_s = 0.0;
        assert!(
            config.bloomcrown.seed_max_age_s >= 3.0 * 3600.0,
            "bloomcrown's bank must outlive the wait"
        );
        let mut world = slab(3, 1, 0.05); // under bloomcrown's establish_pore_min 0.1
        let mut flora = Flora::new(config);
        let site = Site { x: 1, y: 2, z: 0 };
        inject(&mut flora, site, Species::Bloomcrown, 1);

        let first = next_check(site, flora.tick);
        run_to(&mut flora, &mut world, first);
        assert_eq!(flora.view().ledger.establishments, 0, "the gate is shut");
        assert_eq!(whole_seeds(&flora, site, Species::Bloomcrown).round(), 1.0);

        // Two hours pass (the fixture moves the clock without running the model), and the
        // ground is wetted between checks.
        flora.tick += 2 * HOUR_TICKS;
        wet(&mut world, 1, 0, 0.6);
        let open = next_check(site, flora.tick);
        run_to(&mut flora, &mut world, open - 1);
        assert_eq!(
            flora.view().ledger.establishments,
            0,
            "a site is not tested between its checks"
        );
        flora.step(&mut world);
        assert_eq!(flora.tick, open);
        assert_eq!(flora.view().ledger.establishments, 1, "it germinates at the check");
        let born = flora.view().stand_at(site).expect("a stand");
        assert_eq!(born.species, Species::Bloomcrown);
        assert_eq!(born.wood, flora.config.bloomcrown.alive_min);
        assert_eq!(whole_seeds(&flora, site, Species::Bloomcrown), 0.0);
        assert_eq!(flora.view().ledger.seeds_germinated[Species::Bloomcrown.index()], 1);
        assert_conserved(&flora, "after the germination");
    }

    /// Test 2. Attrition kills **whole seeds** — a cohort always holds a whole number of
    /// packages — and every seed it kills is in the litter with its mineral.
    #[test]
    fn attrition_removes_whole_seeds_and_conserves_organic_into_litter() {
        let mut config = FloraConfig::default();
        // The test's own rate: about four tenths of the seeds die per 30 s check.
        config.bloomcrown.seed_attrition_per_s = 1.0 / 60.0;
        let package = package_of(&config.bloomcrown);
        let mut world = slab(3, 1, 0.05); // shut, so nothing germinates
        let mut flora = Flora::new(config);
        let site = Site { x: 1, y: 2, z: 0 };
        inject(&mut flora, site, Species::Bloomcrown, 40);

        let mut before = 40.0;
        let mut died_total = 0.0;
        let mut some_died_some_lived = false;
        for _ in 0..3 {
            let litter0 = flora.view().ground_at(site).unwrap().litter;
            let lmin0 = flora.view().ground_at(site).unwrap().litter_mineral;
            let t = next_check(site, flora.tick);
            run_to(&mut flora, &mut world, t);
            let now = whole_seeds(&flora, site, Species::Bloomcrown);
            assert!((now - now.round()).abs() < 1e-9, "a fraction of a seed: {now}");
            let died = before - now.round();
            assert!(died >= 0.0);
            some_died_some_lived |= died > 0.0 && now.round() > 0.0;
            let g = flora.view().ground_at(site).unwrap();
            assert!(
                (g.litter - litter0 - died * package).abs() < 1e-12,
                "{died} dead seeds are {} of litter, not {}",
                died * package,
                g.litter - litter0
            );
            let n_tissue = flora.config.bloomcrown.n_tissue;
            assert!((g.litter_mineral - lmin0 - died * package * n_tissue).abs() < 1e-12);
            died_total += died;
            before = now.round();
            assert_conserved(&flora, "after a check");
        }
        assert!(some_died_some_lived, "the draws were all or nothing");
        assert_eq!(
            flora.view().ledger.seeds_died[Species::Bloomcrown.index()] as f64,
            died_total
        );
    }

    /// Test 3. The wind kernel: fat-tailed and isotropic. Most seeds within three hops,
    /// a few percent past ten, never the donor's own column, and capped at the world.
    #[test]
    fn the_wind_kernel_is_mostly_near_with_a_few_percent_past_ten_hops() {
        let hop = 2u32;
        let n = 20_000u64;
        let (mut near, mut far) = (0u64, 0u64);
        let mut quadrant = [0u64; 4];
        for i in 0..n {
            let mut rng = Rng::keyed(DOMAIN_WIND, 1, i, 0);
            let (dx, dz) = wind_offset(&mut rng, hop, 1e9);
            assert!((dx, dz) != (0, 0), "a wind seed never lands on its own column");
            let d = ((dx * dx + dz * dz) as f64).sqrt();
            if d <= 3.0 * f64::from(hop) {
                near += 1;
            }
            if d > 10.0 * f64::from(hop) {
                far += 1;
            }
            quadrant[usize::from(dx > 0) * 2 + usize::from(dz > 0)] += 1;
        }
        let near = near as f64 / n as f64;
        let far = far as f64 / n as f64;
        assert!(near > 0.6, "only {near} within three hops");
        assert!((0.02..=0.1).contains(&far), "{far} past ten hops");
        for q in quadrant {
            let share = q as f64 / n as f64;
            assert!((0.18..=0.32).contains(&share), "not isotropic: {quadrant:?}");
        }
        // Capped at the world.
        for i in 0..2_000u64 {
            let mut rng = Rng::keyed(DOMAIN_WIND, 2, i, 0);
            let (dx, dz) = wind_offset(&mut rng, hop, 5.0);
            assert!(((dx * dx + dz * dz) as f64).sqrt() <= 6.0, "({dx}, {dz})");
        }
    }

    /// Test 4. Spores land only on sites whose ground passes the species' gate — every
    /// gate but light — and the rain that finds none falls as litter.
    #[test]
    fn spores_land_only_on_sites_that_pass_the_gate() {
        // A checkerboard of 4 × 4 damp blocks: a velvetpad's 3 × 3 root box reads damp
        // enough (0.3) inside a block and too dry across the corners and outside.
        let (width, depth) = (16u32, 16u32);
        let mut world = slab(width, depth, 0.05);
        for z in 0..depth {
            for x in 0..width as i64 {
                if (x as u32 / 4 + z / 4) % 2 == 0 {
                    wet(&mut world, x, z, 0.6);
                }
            }
        }
        let mut config = FloraConfig::default();
        config.velvetpad.dispersal = Dispersal::Spores;
        config.velvetpad.hop = 5;
        let sc = config.velvetpad.clone();
        let view = world.view();
        let home = Site { x: 8, y: 2, z: 8 };
        let mut sky = Vec::new();
        let ground: Vec<Ground> = Vec::new();
        let (mut banked, mut lost) = (0, 0);
        for tick in 0..400u64 {
            match landing(&view, &mut sky, &ground, &sc, home, 11, tick) {
                Landing::Bank(site) => {
                    banked += 1;
                    assert_ne!(site, home);
                    let g = establishment_gates_on_substrate(&view, site, &sc, 0.0, 0.0);
                    assert!(
                        g.pore_ok && g.aeration_ok && g.depth_ok && g.standing_ok,
                        "a spore landed on {site:?}, which fails {g:?}"
                    );
                }
                Landing::Lost(_) => lost += 1,
                Landing::Retry => {}
            }
        }
        assert!(banked > 100, "only {banked} landed");
        assert!(lost > 0, "a sparse habitat should lose some of the rain");
    }

    /// Test 5. Water landings fall only on standing-water margins: settled water on the
    /// face or beside it, and the face no deeper than the species can stand in.
    #[test]
    fn water_landings_fall_only_at_standing_water_margins() {
        let (width, depth) = (14u32, 7u32);
        let config_w = VoxelConfig {
            width,
            height: 8,
            depth,
            voxel_m: 1.0,
            seed: 5,
            rain_m_per_s: 0.0,
            ..VoxelConfig::default()
        };
        let mut world = World::empty(config_w);
        // A pool: its floor one voxel down, x 6..=8, z 2..=4, a metre of water on it.
        let pool = |x: i64, z: u32| (6..=8).contains(&x) && (2..=4).contains(&z);
        for z in 0..depth {
            for x in 0..width as i64 {
                soil(&mut world, x, 1, z, 0.9);
                if !pool(x, z) {
                    soil(&mut world, x, 2, z, 0.5);
                }
            }
        }
        for z in 2..=4u32 {
            for x in 6..=8i64 {
                world.apply(WorldCommand::AddWater {
                    x,
                    y: 2,
                    z,
                    volume_m3: 1.0,
                });
            }
        }
        let mut config = FloraConfig::default();
        config.siphonreed.dispersal = Dispersal::Water;
        config.siphonreed.hop = 6;
        let sc = config.siphonreed.clone();
        let view = world.view();
        let margin = |s: Site| {
            crate::step::standing_water_beside(&view, s) > 0.0
                && view.standing_depth_m(i64::from(s.x), s.y, s.z) <= sc.drown_depth_m
        };
        // Some margins exist, and the pool floor (a metre deep) is not one of them.
        assert!(margin(Site { x: 5, y: 2, z: 3 }));
        assert!(!margin(Site { x: 7, y: 1, z: 3 }));
        assert!(!margin(Site { x: 1, y: 2, z: 3 }));
        let home = Site { x: 2, y: 2, z: 3 };
        let mut sky = Vec::new();
        let ground: Vec<Ground> = Vec::new();
        let mut banked = 0;
        for tick in 0..200u64 {
            match landing(&view, &mut sky, &ground, &sc, home, 11, tick) {
                Landing::Bank(site) => {
                    banked += 1;
                    assert!(margin(site), "a water seed landed off the margin at {site:?}");
                }
                other => panic!("margins are in reach, yet {other:?}"),
            }
        }
        assert_eq!(banked, 200);
    }

    /// Test 6. A runner daughter: on a free neighbouring cell, paid for out of the
    /// parent's reserve, nothing banked, and conserved.
    #[test]
    fn a_runner_daughter_is_adjacent_paid_from_the_reserve_and_conserved() {
        let mut config = FloraConfig::default();
        // The test's own numbers: every package is a runner, and one tick funds one.
        config.springturf.clonal_share = 1.0;
        config.springturf.propagule_rate = 1.0;
        config.springturf.donor_reserve_floor = 0.0;
        let sc = config.springturf.clone();
        let package = package_of(&sc);
        let mut world = slab(5, 1, 0.6);
        let mut flora = Flora::new(config);
        assert!(flora.apply(
            &world,
            crate::Command::Seed {
                x: 2,
                z: 0,
                species: Species::Springturf,
                wood: sc.wood_max,
            }
        ));
        let parent = Site { x: 2, y: 2, z: 0 };
        flora.step(&mut world);

        let v = flora.view();
        assert_eq!(v.stands.len(), 2, "one daughter");
        let daughter = v.stands.iter().find(|s| s.site != parent).expect("a daughter");
        let dx = (i64::from(daughter.site.x) - 2).abs();
        assert_eq!(dx, 1, "adjacent: {:?}", daughter.site);
        assert_eq!(daughter.species, Species::Springturf);
        assert_eq!(daughter.wood, sc.alive_min);
        assert!((daughter.material() - package).abs() < 1e-15);
        let slot = Species::Springturf.index();
        assert_eq!(v.ledger.clonal_births[slot], 1);
        assert_eq!(v.ledger.establishments, 1);
        // Paid out of the reserve: what the parent funded is the package plus what it is
        // still saving.
        let mother = v.stand_at(parent).expect("the parent");
        assert!(
            (v.ledger.propagule_funded[slot] - package - mother.parcel).abs() < 1e-15,
            "funded {} for a {package} package and {} saved",
            v.ledger.propagule_funded[slot],
            mother.parcel
        );
        assert!(v.ground.iter().all(|g| g.seeds.is_empty()), "a runner banks nothing");
        assert_conserved(&flora, "after the runner");
    }

    /// Test 7. A banked site is tested only at its own check — except on the tick a
    /// shower ends, when every bank is.
    #[test]
    fn a_site_is_not_tested_between_checks_except_on_a_shower_flush() {
        let mut config = FloraConfig::default();
        config.bloomcrown.seed_attrition_per_s = 0.0;
        let mut world = slab(4, 1, 0.6); // open: a bloomcrown passes everywhere
        let mut flora = Flora::new(config);

        // Banked on the tick of its own check, so the whole period is between checks.
        let a = Site { x: 1, y: 2, z: 0 };
        let first = next_check(a, 0);
        run_to(&mut flora, &mut world, first);
        inject(&mut flora, a, Species::Bloomcrown, 1);
        run_to(&mut flora, &mut world, first + CHECK_PERIOD_TICKS - 1);
        assert_eq!(flora.view().ledger.establishments, 0, "tested between checks");
        flora.step(&mut world);
        assert_eq!(flora.view().ledger.establishments, 1, "not tested at its check");

        // A second bank, between its checks, and a shower that ends this tick.
        let b = Site { x: 3, y: 2, z: 0 };
        inject(&mut flora, b, Species::Bloomcrown, 1);
        let due = next_check(b, flora.tick);
        assert!(due > flora.tick + 1, "the fixture needs a tick that is not b's check");
        flora.was_raining = true; // the world is dry: the shower has just ended
        flora.step(&mut world);
        assert!(flora.tick < due);
        assert_eq!(flora.view().ledger.establishments, 2, "the flush did not test it");
        assert!(flora.view().stand_at(b).is_some());
        assert_conserved(&flora, "after the flush");
    }
}
