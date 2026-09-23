// R2a repair 1 verification at af6808e: expected to fail.
// Copy to crates/cubarium-search/tests/review_r2a_seam_temporary.rs, then run
// cargo test -p cubarium-search --release --test review_r2a_seam_temporary
use cubarium_core::neural::{Policy, gru::Gru32};
use cubarium_search::es::{self, episode::Limits};
use std::sync::atomic::AtomicBool;

#[test]
fn seam_diagnostic_must_count_physical_turn_or_mark_it_unmeasured() {
    let mut layout = es::training_layouts()[0].clone();
    layout.start = (8, 15);
    layout.heading = (0.0, 1.0);
    let mut weights = Gru32::zeros();
    weights.b_o[0] = 8.0;
    weights.b_o[1] = 0.12;
    weights.b_o[2] = -8.0;
    weights.b_o[3] = -8.0;
    weights.b_o[4] = -8.0;
    let driver = es::Driver::Policy(Box::new(Policy::new(weights)));
    let cancel = AtomicBool::new(false);
    let seam = es::episode::run(&layout, &driver, 40, Limits::new(&cancel), "seam").unwrap();
    layout.start = (8, 8);
    let flat = es::episode::run(&layout, &driver, 40, Limits::new(&cancel), "flat").unwrap();
    assert!(seam.alive && flat.alive);
    assert!(seam.seam_crossing_ticks > 0);
    eprintln!("flat sweep {}, seam sweep {}, crossings {}, unmeasured {}, flat motor {}, seam motor {}",
        flat.turn_sweep_rad, seam.turn_sweep_rad, seam.seam_crossing_ticks,
        seam.turn_unmeasured_ticks, flat.motion_billed, seam.motion_billed);
    assert!((flat.turn_sweep_rad - seam.turn_sweep_rad).abs() < 1e-9
        || seam.turn_unmeasured_ticks >= seam.seam_crossing_ticks,
        "physical turn on crossing ticks is neither counted nor marked unmeasured");
}
