use cubarium_voxel_flora::{FloraConfig, Species};
use cubarium_voxel_sim::params::Params;
use cubarium_webmodel::flora::{
    census::{self, Face, StandRec},
    climate::DayModel,
    kernel::{Kernel, RunnerBox},
    model::{Model, Params as ModelParams},
    zones,
};

#[test]
fn saved_food_and_surface_moisture_survive_the_census_and_model_start() {
    let fc = FloraConfig::default();
    let mut face = Face::bare(0, 1, 0, 1.0, 1.0);
    face.pore = 0.7;
    let sp = Species::Bloomcrown;
    let sc = fc.species(sp);
    let nectar = sc.nectar_cap * 0.6;
    let fruit = sc.fruit_cap * 0.4;
    face.plant = Some((
        sp,
        StandRec {
            wood: sc.wood_max,
            foliage: sc.alpha * sc.wood_max,
            nectar,
            fruit,
            fruit_age_s: 17.0,
            fruit_rest_s: 23.0,
            ..Default::default()
        },
    ));
    let row = census::face_row(&face);
    let read = census::face_of_row(&row).unwrap();
    assert_eq!(read.pore, face.pore);
    assert_eq!(read.plant.unwrap().1.nectar, nectar);
    assert_eq!(read.plant.unwrap().1.fruit, fruit);
    let zs = zones::build(&fc, &[vec![read.clone()]]);
    let nz = zs.zones.len();
    let kernels = (0..Species::COUNT)
        .map(|_| Kernel {
            land: vec![vec![1.0; nz]; nz],
            lost: vec![0.0; nz],
        })
        .collect();
    let runners = (0..Species::COUNT)
        .map(|_| RunnerBox {
            faces: 1.0,
            same_zone: 1.0,
        })
        .collect();
    let p = Params::defaults();
    let m = Model::new(
        fc.clone(),
        DayModel::of(&p.world, fc.upkeep_q10),
        zs,
        kernels,
        runners,
        &[read],
        0.0,
        ModelParams::default(),
    );
    assert_eq!(m.offer().nectar, nectar);
    assert_eq!(m.offer().fruit, fruit);
    assert_eq!(m.z[0].cohorts[sp.index()][0].st.fruit_age_s, 17.0);
    assert_eq!(m.z[0].cohorts[sp.index()][0].st.fruit_rest_s, 23.0);
}

#[test]
fn a_shore_egg_drowns_before_hatching_and_returns_its_parcel_to_litter() {
    use cubarium_voxel_fauna::{EggSite, Founder};
    use cubarium_webmodel::fauna::{
        behavior::{Habitat, ShoreBand},
        EggCohort, FaunaModel, Fits,
    };
    let mut p = Params::defaults();
    let f = Founder::RippleSnail;
    p.fauna.founders[f.index()].egg_site = EggSite::Shore;
    p.fauna.founders[f.index()]
        .amphibious
        .as_mut()
        .unwrap()
        .egg_drown_s = 0.5;
    p.fauna.founders[f.index()].core.reproduction.incubation_s = 10.0;
    let mut m = FaunaModel::new(&p, Fits::unfitted(), 1.0);
    m.habitat = Some(Habitat {
        area: 1.0,
        shore: vec![ShoreBand {
            faces: 1.0,
            depth: 0.1,
            moisture: 1.0,
            ..Default::default()
        }],
        ..Default::default()
    });
    let parcel = p.fauna.founder(f).core.reproduction.egg_organic;
    m.eggs.push(EggCohort {
        lineage: f,
        n: 2.0,
        organic: parcel,
        ..Default::default()
    });
    let (_, ret) = m.step(&Default::default());
    assert_eq!(m.ledger[f.index()].eggs_drowned, 2.0);
    assert_eq!(m.ledger[f.index()].hatched, 0.0);
    assert!((ret.litter - 2.0 * parcel).abs() < 1e-12);
    assert_eq!(m.organic_held(), 0.0);
}

#[test]
fn a_lit_lure_has_visits_and_kills_without_a_fitted_bellwing_kill_rate() {
    use cubarium_voxel_fauna::Founder;
    use cubarium_webmodel::fauna::{behavior::Habitat, Cohort, FaunaModel, Fits};
    let mut p = Params::defaults();
    let lj = Founder::Lanternjaw;
    let bw = Founder::Bellwing;
    p.fauna.founders[lj.index()]
        .hunter
        .as_mut()
        .unwrap()
        .prey_max_ratio = 10.0;
    p.fauna.founders[lj.index()]
        .hunter
        .as_mut()
        .unwrap()
        .strike_power = 1.0;
    p.fauna.founders[lj.index()]
        .core
        .reproduction
        .surplus_hold_s = 100.0;
    let mut m = FaunaModel::new(&p, Fits::unfitted(), 1.0);
    m.habitat = Some(Habitat {
        area: 1.0,
        ..Default::default()
    });
    m.lure_lit_override = Some(1.0);
    let mut h = Cohort::new(1.0, p.fauna.founder(lj).core.body_max, 0.0);
    h.b.eating = true;
    let mut prey = Cohort::new(2.0, p.fauna.founder(bw).core.body_max, 0.0);
    prey.b.eating = true;
    m.add(lj, h);
    m.add(bw, prey);
    let before = m.organic_held();
    let (_, ret) = m.step(&Default::default());
    assert!(m.ledger[lj.index()].lure_visits > 0.0);
    assert!(m.ledger[bw.index()].killed_by[lj.index()] > 0.0);
    let respired: f64 = m.ledger.iter().map(cubarium_webmodel::web::respired).sum();
    assert!(
        (m.organic_held() + ret.litter + ret.carrion + respired + m.carcass_decayed - before).abs()
            < 1e-10
    );
}

#[test]
fn intake_clamping_one_food_preserves_other_lineages_feeding() {
    use cubarium_voxel_fauna::{Food, Founder};
    use cubarium_webmodel::fauna::{Cohort, FaunaModel, Fits};
    let p = Params::defaults();
    let f = Founder::Blind;
    let mut m = FaunaModel::new(&p, Fits::unfitted(), 1.0);
    m.add(f, Cohort::new(1.0, p.fauna.founder(f).core.body_max, 0.0));
    let mut clamp = [[None; Food::COUNT]; Founder::COUNT];
    clamp[Founder::Bellwing.index()][Food::Nectar.index()] = Some(0.0);
    m.clamped_intake = Some(clamp);
    m.step(&cubarium_webmodel::exchange::Supply {
        litter: 1.0,
        ..Default::default()
    });
    assert!(m.ledger[f.index()].swallowed[Food::Litter.index()] > 0.0);
}

#[test]
fn shore_egg_exposure_is_not_averaged_away_by_cohort_merging() {
    use cubarium_voxel_fauna::Founder;
    use cubarium_webmodel::fauna::{EggCohort, FaunaModel, Fits};
    let p = Params::defaults();
    let f = Founder::RippleSnail;
    let mut m = FaunaModel::new(&p, Fits::unfitted(), 1.0);
    let e = EggCohort {
        lineage: f,
        n: 1.0,
        organic: p.fauna.founder(f).core.reproduction.egg_organic,
        ..Default::default()
    };
    m.eggs = vec![
        e.clone(),
        EggCohort {
            wet_s: p.fauna.founder(f).amphibious.unwrap().egg_drown_s,
            ..e
        },
    ];
    m.merge_cohorts();
    assert_eq!(m.eggs.len(), 2);
}

#[test]
fn film_spreads_to_a_cardinal_neighbor_and_preserves_its_pool() {
    let mut fc = FloraConfig::default();
    fc.film_growth_per_s = 0.0;
    fc.film_land_growth_per_s = 0.0;
    fc.film_maintenance_per_s = 0.0;
    fc.film_dry_death_per_s = 0.0;
    fc.film_spread_per_s = 0.01;
    let mut wet = Face::bare(1, 1, 0, 1.0, 1.0);
    wet.depth_m = fc.film_wet_min_depth_m;
    wet.film = 0.5 * fc.film_capacity_per_m2 * fc.voxel_m.powi(2);
    let near = Face::bare(2, 1, 0, 1.0, 1.0);
    let far = Face::bare(4, 1, 0, 1.0, 1.0);
    let faces = vec![wet, near, far];
    let zs = zones::build(&fc, &[faces.clone()]);
    let nz = zs.zones.len();
    let kernels = (0..Species::COUNT)
        .map(|_| Kernel {
            land: vec![vec![1.0; nz]; nz],
            lost: vec![0.0; nz],
        })
        .collect();
    let runners = (0..Species::COUNT)
        .map(|_| RunnerBox {
            faces: 1.0,
            same_zone: 1.0,
        })
        .collect();
    let p = Params::defaults();
    let mut m = Model::new(
        fc.clone(),
        DayModel::of(&p.world, fc.upkeep_q10),
        zs,
        kernels,
        runners,
        &faces,
        0.0,
        ModelParams::default(),
    );
    let before = m.film;
    m.grow_film(1.0);
    let land: f64 = m.film_beds.iter().filter(|b| b.land).map(|b| b.stock).sum();
    assert!(
        (land - before * fc.film_spread_per_s).abs() < 1e-12,
        "only one adjacent face receives a flux"
    );
    assert!((m.film - before).abs() < 1e-12);
    let mut off = m.clone();
    off.config.film_spread_per_s = 0.0;
    let land_before: f64 = off
        .film_beds
        .iter()
        .filter(|b| b.land)
        .map(|b| b.stock)
        .sum();
    off.grow_film(1.0);
    assert_eq!(
        off.film_beds
            .iter()
            .filter(|b| b.land)
            .map(|b| b.stock)
            .sum::<f64>(),
        land_before
    );
}

#[test]
fn pried_intervals_keep_their_higher_kill_hazard_in_a_pooled_cohort() {
    use cubarium_voxel_fauna::{Founder, FounderPhysiology};
    use cubarium_webmodel::fauna::behavior;
    let mut h = FounderPhysiology::frozen(Founder::Chorister);
    let mut p = FounderPhysiology::frozen(Founder::Browser);
    h.hunter.as_mut().unwrap().strike_power = 0.1;
    p.hunker.enabled = true;
    p.hunker.shield = 0.75;
    p.armour = 0.25;
    let mass = p.core.birth_body;
    let guarded = behavior::guarded_kill_share(&h, mass, &p, mass, 1.0, 0.0);
    let bared = behavior::guarded_kill_share(&h, mass, &p, mass, 1.0, 1.0);
    let mixed = behavior::guarded_kill_share(&h, mass, &p, mass, 1.0, 0.5);
    let mean_hit = behavior::guarded_strike_share(&h, mass, &p, mass, 1.0, 0.5);
    assert!(guarded < mixed && mixed < bared);
    assert!(
        mixed > mean_hit * mean_hit,
        "averaging damage first loses the bared interval's hazard"
    );
    p.hunker.enabled = false;
    assert_eq!(
        behavior::guarded_kill_share(&h, mass, &p, mass, 1.0, 0.5),
        behavior::guarded_kill_share(&h, mass, &p, mass, 0.0, 0.0)
    );
}
