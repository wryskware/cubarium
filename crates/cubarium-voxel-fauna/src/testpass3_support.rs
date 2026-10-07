//! Small hand-built fixtures shared only by the independent third pass.
use crate::{Command, Fauna, Founder, StartingStores};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WC, Config, Material, World};
use cubarium_voxel_flora::{Flora, Site};

pub(crate) const V: f64 = 0.25;

pub(crate) fn plain(width: u32, depth: u32, height: u32) -> World {
    let mut w = World::empty(Config {
        width,
        depth,
        height,
        voxel_m: V,
        day: DayClock {
            mode: DayMode::Off,
            ..Default::default()
        },
        ..Default::default()
    });
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=2 {
                w.apply(WC::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    w
}

pub(crate) fn introduce(fauna: &mut Fauna, w: &World, f: Founder, x: i64, z: u32) -> usize {
    assert!(
        fauna.apply(
            w,
            Command::IntroduceFounder {
                x,
                z,
                founder: f,
                stores: StartingStores {
                    body: 0.3,
                    reserve: 0.4
                },
                heading_rad: 0.0,
            }
        ),
        "fixture: introduce {}",
        f.name()
    );
    fauna.animals.len() - 1
}

pub(crate) fn place(fauna: &mut Fauna, i: usize, s: Site, heading: f64) {
    let a = &mut fauna.animals[i];
    a.site = s;
    a.pose.x = (f64::from(s.x) + 0.5) * V;
    a.pose.z = (f64::from(s.z) + 0.5) * V;
    a.pose.y_m = crate::surface_m(s.y, V);
    a.pose.heading_rad = heading;
}

pub(crate) fn close(got: f64, want: f64) {
    assert!(
        (got - want).abs() < 1e-10 * want.abs().max(1.0),
        "got {got}, expected {want}"
    );
}

pub(crate) fn ledgers(fauna: &Fauna, flora: &Flora) {
    let (av, fv) = (fauna.view(), flora.view());
    for (got, want) in [
        (av.organic(), av.ledger.expected_organic()),
        (av.mineral(), av.ledger.expected_mineral()),
        (av.energy(), av.ledger.expected_energy()),
        (fv.organic(), fv.ledger.expected_organic()),
        (fv.mineral(), fv.ledger.expected_mineral()),
        (fv.energy(), fv.ledger.expected_energy()),
    ] {
        close(got, want);
    }
}
