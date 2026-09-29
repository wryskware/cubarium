//! P1 (predation substrate), tests first: carrion decays at the new rate and the ground
//! pool carries a carcass record (`design/predation-and-flight-2026-09-28.md` §2.5, and
//! decision 2; brief P1 case 9 and the flora half of case 14). Red by design until the
//! implementation commit adds:
//!
//! - `FloraConfig::carrion_decomposition` at 0.00028 /s (an e-fold of about an hour),
//!   in the default and the voxel-size presets;
//! - `Carcass { lineage: u8, heading_rad: f64, organic: f64 }` (lib.rs, deriving `Clone,
//!   Copy, Debug, PartialEq, Serialize, Deserialize`): the dead animal's lineage code (the
//!   fauna's `Founder::index`), its heading and the organic matter it died with;
//! - `Carcass::remains(&self, carrion: f64) -> bool`: the pool is below 30 % of what the
//!   body died with, when the presenter draws remains instead of the body;
//! - `Ground::carcass: Option<Carcass>` and `Flora::lay_carcass(&mut self, site: Site,
//!   carcass: Carcass) -> bool`: the record carries no mass, the pool does;
//! - the flora snapshot `SCHEMA` bumped past 8 for the new field.

use crate::snapshot::{SCHEMA, decode, encode};
use crate::{Carcass, Deposit, DepositKind, Flora, FloraConfig, Site};
use cubarium_voxel::DT;

const SITE: Site = Site { x: 3, y: 2, z: 1 };

fn corpse(flora: &mut Flora, organic: f64) {
    assert!(flora.deposit(
        SITE,
        Deposit {
            kind: DepositKind::Carrion,
            organic,
            mineral: 0.02 * organic,
            energy: 2.0 * organic,
        }
    ));
}

fn assert_closes(flora: &Flora, when: &str) {
    let v = flora.view();
    for (held, expected, what) in [
        (v.organic(), v.ledger.expected_organic(), "organic"),
        (v.mineral(), v.ledger.expected_mineral(), "mineral"),
        (v.energy(), v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            (held - expected).abs() <= 1e-12 * held.abs().max(1e-6),
            "{when}: flora {what} holds {held}, the ledger expects {expected}"
        );
    }
}

/// Decision 2: carrion's e-fold goes from 200 s to about an hour, in every preset.
#[test]
fn carrion_decomposes_with_an_e_fold_of_about_an_hour() {
    for c in [
        FloraConfig::default(),
        FloraConfig::for_voxel_size(0.25),
        FloraConfig::for_voxel_size(0.125),
    ] {
        assert_eq!(c.carrion_decomposition, 0.00028);
        let e_fold_h = 1.0 / c.carrion_decomposition / 3600.0;
        assert!((e_fold_h - 1.0).abs() < 0.01, "e-fold {e_fold_h} h");
    }
}

/// Case 9: a corpse decays at `k · dt` of the pool per tick, its organic matter respired
/// and its mineral released to the soil at the pool's own fraction, as before; the books
/// close.
#[test]
fn carrion_decays_at_the_new_rate_and_its_mineral_goes_to_the_soil() {
    let mut flora = Flora::new(FloraConfig::default());
    corpse(&mut flora, 0.05);
    let g0 = flora.view().ground_at(SITE).expect("the pool").clone();
    let respired0 = flora.view().ledger.respired_out;
    let k = flora.view().config.carrion_decomposition;
    const TICKS: i32 = 200;
    for _ in 0..TICKS {
        flora.decompose_dead_pools();
    }
    let g = flora.view().ground_at(SITE).expect("the pool").clone();
    let want = g0.carrion * (1.0 - k * DT).powi(TICKS);
    assert!(
        (g.carrion - want).abs() <= 1e-12 * want,
        "carrion {} after {TICKS} ticks, the rate gives {want}",
        g.carrion
    );
    let gone = g0.carrion - g.carrion;
    assert!(
        ((flora.view().ledger.respired_out - respired0) - gone).abs() <= 1e-15,
        "what left the pool was respired"
    );
    // Mineral: out of the carrion and into the soil, at the pool's own fraction.
    let released = g0.carrion_mineral - g.carrion_mineral;
    assert!(((g.mineral - g0.mineral) - released).abs() <= 1e-15);
    assert!((released - 0.02 * gone).abs() <= 1e-12 * released);
    assert!((g.carrion_mineral / g.carrion - 0.02).abs() < 1e-12);
    assert_closes(&flora, "decayed");
}

/// The carcass record: laid on the pool, read back, massless, drawn as remains below
/// 30 % of what the body died with, and kept by a snapshot.
#[test]
fn the_ground_pool_carries_a_massless_carcass_record() {
    assert!(SCHEMA >= 9, "the ground record changed: schema {SCHEMA}");
    let mut flora = Flora::new(FloraConfig::default());
    corpse(&mut flora, 0.05);
    let (o, m, e) = {
        let v = flora.view();
        (v.organic(), v.mineral(), v.energy())
    };
    let ledger = flora.view().ledger.clone();
    let record = Carcass {
        lineage: 1,
        heading_rad: 0.75,
        organic: 0.05,
    };
    assert!(flora.lay_carcass(SITE, record));
    assert_eq!(flora.view().ground_at(SITE).unwrap().carcass, Some(record));
    {
        let v = flora.view();
        assert_eq!((v.organic(), v.mineral(), v.energy()), (o, m, e), "the record has mass");
    }
    assert_eq!(*flora.view().ledger, ledger, "laying a record books nothing");
    assert_closes(&flora, "carcass laid");

    assert!(!record.remains(0.31 * 0.05), "above 30 % it is still the body");
    assert!(record.remains(0.29 * 0.05), "below 30 % it is remains");

    let back = decode(&encode(&flora)).expect("round trip");
    assert_eq!(back.view().ground_at(SITE).unwrap().carcass, Some(record));
}
