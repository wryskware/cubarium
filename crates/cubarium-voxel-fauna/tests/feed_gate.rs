//! The heuristics' feed gate (mass round, 2026-09-29): both foragers feed on any food the
//! mouth tastes (cue above their own `feed_gate`, the test's own 0.02) and not on none or
//! crumbs below it. No ticks.

use cubarium_voxel_fauna::{
    BlindForager, BrowserForager, Controller, Founder, TeacherConfig, Response,
};

fn tasting(founder: Founder, cue: f64) -> Vec<f64> {
    let m = founder.manifest();
    let mut o = vec![0.0; m.inputs()];
    o[7] = 1.0;
    let t = m.modules.iter().find(|m| m.name == "Taste(1)").expect("taste");
    o[t.offset] = cue;
    o[t.offset + 2] = 1.0;
    o
}

fn feeds(c: &mut dyn Controller, founder: Founder, cue: f64) -> bool {
    match c.drive(&tasting(founder, cue)) {
        Response::Bounded(a) => a.feed > 0.5,
        Response::Logits(_) => unreachable!(),
    }
}

#[test]
fn both_foragers_feed_on_any_food_they_taste() {
    let cfg = TeacherConfig {
        feed_gate: 0.02,
        ..TeacherConfig::default()
    };
    // The two grazing foragers; the lanternjaw's ambush feeds by its own rule.
    for founder in [Founder::Blind, Founder::Browser] {
        let mut c: Box<dyn Controller> = match founder {
            Founder::Blind => Box::new(BlindForager::with_config(cfg)),
            Founder::Browser => Box::new(BrowserForager::with_config(cfg)),
            _ => unreachable!(),
        };
        assert!(feeds(c.as_mut(), founder, 0.05), "{founder:?}: a cue over the gate");
        assert!(!feeds(c.as_mut(), founder, 0.0), "{founder:?}: nothing tasted");
        assert!(!feeds(c.as_mut(), founder, 0.01), "{founder:?}: crumbs under the gate");
    }
}
