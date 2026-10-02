//! The heuristics' feed gate under the mass-scaled taste (mass round, 2026-09-29): taste's
//! reference is `M_EMIT x mass_scale`, so at the shipped x30 a whole springturf or a
//! starter litter tile tastes ~0.05. Both foragers feed on any food the mouth tastes
//! (cue above `feed_gate`, 0.02) and not on none. No ticks.

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
    assert_eq!(TeacherConfig::default().feed_gate, 0.02);
    // The two grazing foragers; the lanternjaw's ambush feeds by its own rule.
    for founder in [Founder::Blind, Founder::Browser] {
        let mut c: Box<dyn Controller> = match founder {
            Founder::Blind => Box::new(BlindForager::new()),
            Founder::Browser => Box::new(BrowserForager::new()),
            _ => unreachable!(),
        };
        assert!(feeds(c.as_mut(), founder, 0.05), "{founder:?}: a x30 turf or tile");
        assert!(!feeds(c.as_mut(), founder, 0.0), "{founder:?}: nothing tasted");
        assert!(!feeds(c.as_mut(), founder, 0.01), "{founder:?}: crumbs");
    }
}
