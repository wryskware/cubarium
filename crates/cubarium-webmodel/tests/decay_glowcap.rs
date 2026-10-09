//! Short checks for the webmodel's mirrored decay and saprotroph sun laws.
use cubarium_voxel_flora::{DT, FloraConfig, Species};
use cubarium_webmodel::flora::census::Face;
use cubarium_webmodel::flora::climate::{Clock, DayModel};
use cubarium_webmodel::flora::kernel::{Kernel, RunnerBox};
use cubarium_webmodel::flora::model::{Model, NS, Params, Weather};
use cubarium_webmodel::flora::stand::{self, Death, StandState};

#[test]
fn sunlit_fungal_stock_retreats_to_litter_despite_income_and_shade_can_grow() {
    let mut config = FloraConfig::default();
    let sc = &mut config.glowcap;
    sc.wood_max = 1.0;
    sc.alive_min = 0.1;
    sc.alpha = 0.5;
    sc.reserve_cap = 0.0;
    sc.maintenance = 0.0;
    sc.senescence = 0.0;
    sc.wood_rate = 2.0 / DT;
    sc.foliage_rate = 2.0 / DT;
    sc.sun_shade_max = 0.5;
    sc.sun_dark_shelter = 0.0;
    sc.sun_cap_loss_per_s = 0.5 / DT;
    sc.sun_mycelium_loss_per_s = 0.2 / DT;
    sc.dark_uptake_per_s = 0.0;
    sc.substrate_required = true;
    let clock = Clock {
        daylight: 1.0,
        upkeep: 1.0,
        night: 1.0,
        to_dawn_s: None,
        dt: DT,
    };
    for light in [0.0, 1.0] {
        let mut st = StandState {
            wood: 0.5,
            foliage: 0.1,
            ..Default::default()
        };
        let initial = st.wood;
        let mut died = None;
        for _ in 0..5 {
            let before = st.material();
            let site = stand::Site {
                light_eff: light,
                available: 1.0,
                saturated: 0.0,
                mineral: 1.0,
                darkness: 0.0,
                substrate_taken: 2.0,
                substrate_gross: 1.0,
            };
            let (flows, death) = stand::grow(sc, &mut st, site, clock);
            let expected = before + site.substrate_taken + flows.seep
                - flows.respired
                - flows.litter
                - flows.dead_wood;
            assert!(
                (st.material() - expected).abs() < 1e-12,
                "sun losses and unused income are fully booked: {} vs {expected}",
                st.material()
            );
            if light > sc.sun_shade_max {
                assert!(
                    st.wood < initial,
                    "full sun cannot be outgrown with substrate"
                );
                assert!(flows.litter > 0.0, "desiccated stock goes to litter");
            } else {
                assert!(st.wood > initial, "shade allows saprotroph growth");
            }
            if death.is_some() {
                died = death;
                break;
            }
        }
        assert_eq!(
            died,
            if light > sc.sun_shade_max {
                Some(Death::Desiccated)
            } else {
                None
            }
        );
    }
}

fn bare_model() -> Model {
    let mut config = FloraConfig::default();
    config.decomposition = 0.1;
    config.litter_fragment_boost = 0.8;
    config.litter_fragment_duration_s = 4.0 * DT;
    let faces = vec![Face::bare(0, 1, 0, 1.0, 1.0)];
    let zones = cubarium_webmodel::flora::zones::build(&config, &[faces.clone()]);
    let kernels = (0..NS)
        .map(|_| Kernel {
            land: vec![vec![1.0]],
            lost: vec![0.0],
        })
        .collect();
    let runners = (0..NS)
        .map(|_| RunnerBox {
            faces: 1.0,
            same_zone: 1.0,
        })
        .collect();
    let day = DayModel::of(&cubarium_voxel::Config::default(), config.upkeep_q10);
    let mut model = Model::new(
        config,
        day,
        zones,
        kernels,
        runners,
        &faces,
        0.0,
        Params {
            dt: DT,
            weather: Weather::Frozen,
            ..Default::default()
        },
    );
    model.z[0].litter = 2.0;
    model
}

fn organic_closes(model: &Model, initial: f64) {
    let c = &model.c;
    let taken = c.taken.litter
        + c.taken.foliage.iter().sum::<f64>()
        + c.taken.carrion
        + c.taken.film
        + c.taken.nectar
        + c.taken.fruit;
    let expected = initial + c.fixed + c.seep + c.returned - c.respired - c.decomposed - taken;
    assert!(
        (model.organic() - expected).abs() < 1e-9,
        "organic stock {} vs booked {expected}",
        model.organic()
    );
}

#[test]
fn a_zone_where_litter_was_taken_decays_faster_then_returns_to_ordinary_decay() {
    let (mut fed, mut control) = (bare_model(), bare_model());
    let initial = fed.organic();
    fed.take.litter = 0.01;
    fed.returns.litter = 0.01;
    fed.step();
    control.step();
    assert_eq!(fed.z[0].fragmented_s, fed.config.litter_fragment_duration_s);
    assert!(
        (fed.z[0].litter - control.z[0].litter).abs() < 1e-12,
        "the bite and return leave equal litter pools"
    );
    fed.take.litter = 0.0;
    fed.returns.litter = 0.0;
    let mut excess_before = f64::INFINITY;
    for _ in 0..5 {
        let (a, b) = (fed.z[0].litter, control.z[0].litter);
        fed.step();
        control.step();
        let excess = (a - fed.z[0].litter) / a - (b - control.z[0].litter) / b;
        assert!(
            excess >= -1e-12 && excess < excess_before,
            "the fragmentation boost fades: {excess} after {excess_before}"
        );
        excess_before = excess + 1e-13;
        organic_closes(&fed, initial);
        organic_closes(&control, initial);
    }
    assert!(excess_before.abs() < 1e-12);
    assert_eq!(fed.z[0].fragmented_s, 0.0);
    assert_eq!(
        fed.stands()[Species::Glowcap.index()].0,
        0.0,
        "bare decay needs no fungus"
    );
}
