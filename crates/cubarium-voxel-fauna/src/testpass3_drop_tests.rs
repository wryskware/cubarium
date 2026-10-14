//! Independent DROP contracts, written before consulting the package tests:
//! A trapped grounded walker waits its configured delay, then takes the lowest reachable
//! landing beyond its step and within its size-scaled reach. Disabling reach clears plans.
//! Ordinary routes never drop. The landing adds only the configured excess-height wound;
//! accumulated wound can kill as Fell, with organic/mineral/energy ledgers closed.
//! Calm watching must not prevent a trapped prey's escape; a shore ledge may be a landing.

use super::{DropPlan, DropRule, Trapped, reflex, take_off};
use crate::testpass3_support::{V, close, introduce, ledgers, place, plain};
use crate::{Actions, Activity, Departure, Fauna, FaunaConfig, Founder, MobilityMode};
use cubarium_voxel::{Command as WC, DT, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};
use std::f64::consts::FRAC_PI_2;

const SP: Founder = Founder::Seedporter;

fn fixture() -> (World, Flora, Fauna) {
    let mut w = plain(48, 16, 24);
    // A young body fits on the pocket, the adult route graph does not.
    for (xs, zs, ys) in [
        (14..=19, 4..=8, 3..=3),
        (20..=20, 8..=8, 3..=5),
        (21..=23, 4..=12, 3..=11),
        (14..=23, 9..=11, 3..=11),
    ] {
        for z in zs {
            for x in xs.clone() {
                for y in ys.clone() {
                    w.apply(WC::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Rock,
                    });
                }
            }
        }
    }
    let mut c = FaunaConfig::default();
    let p = &mut c.founders[SP.index()];
    p.active = Activity::Always;
    p.glider = None;
    p.adult_length_m = 0.4;
    p.adult_width_m = 0.3; // adult disc exceeds the pocket's half-voxel clearance
    p.adult_height_m = 0.2;
    p.core.body_min = 0.01 * p.core.body_max;
    p.step_up_m = V;
    p.step_down_m = V;
    p.drop = DropRule {
        after_s: 4.0 * DT,
        max_m: 3.0,
        wound_per_m: 0.4,
        water_max_m: 0.0,
    };
    c.predation.wound_heal_per_s = 0.0;
    c.teachers.route_refresh_s = 20.0 * DT;
    c.teachers.way_on_m = 2.0;
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    introduce(&mut f, &w, SP, 4, 2);
    place(&mut f, 0, Site { x: 20, y: 5, z: 8 }, -FRAC_PI_2);
    (w, Flora::new(FloraConfig::default()), f)
}

fn arm(f: &mut Fauna) -> DropPlan {
    let a = f.animals[0];
    let plan = DropPlan {
        from: a.site,
        land: Site { x: 19, y: 3, z: 8 },
        edge: Site { x: 19, y: 5, z: 8 },
        at: 0,
    };
    f.animals[0].pose.x = 20.0 * V + 1e-4;
    f.drop_plans.insert(a.id, plan);
    f.trapped.insert(
        a.id,
        Trapped {
            since: 0,
            checked: 0,
            site: a.site,
            cornered: true,
            aloft: None,
        },
    );
    plan
}

fn go() -> Actions {
    Actions {
        forward: 1.0,
        turn: 0.0,
        feed: 0.0,
    }
}

#[test]
fn trapped_delay_is_inclusive_on_the_exact_controller_sample() {
    let (w, flora, mut f) = fixture();
    assert!(reflex(&mut f, 0, &w.view(), &flora.view()).is_none());
    let id = f.animals[0].id;
    assert!(
        f.trapped[&id].cornered,
        "fixture is truly trapped, no glide or walking exit"
    );
    let due = (f.config.founder(SP).drop.after_s / DT).round() as u64;
    f.tick = due - 1;
    assert!(reflex(&mut f, 0, &w.view(), &flora.view()).is_none());
    f.tick = due;
    assert!(reflex(&mut f, 0, &w.view(), &flora.view()).is_some());
    assert!(f.drop_plans.contains_key(&id));
}

#[test]
fn disabling_drop_reach_clears_clock_and_pending_landing() {
    let (w, flora, mut f) = fixture();
    arm(&mut f);
    let id = f.animals[0].id;
    f.config.founders[SP.index()].drop.max_m = 0.0;
    assert!(reflex(&mut f, 0, &w.view(), &flora.view()).is_none());
    assert!(!f.drop_plans.contains_key(&id) && !f.trapped.contains_key(&id));
    take_off(&mut f, 0, SP, &w.view(), go());
    assert_eq!(f.ledger.drops_by_founder[SP.index()], 0);
}

/// Leaving the ground clears the pending jump; the trapped clock outlives a brief time off
/// its feet (SP-STUCK: a climb and a cling restarted it, so it never ran out).
#[test]
fn leaving_ground_clears_a_pending_drop() {
    let (w, flora, mut f) = fixture();
    arm(&mut f);
    let id = f.animals[0].id;
    f.animals[0].mobility.mode = MobilityMode::Perched;
    assert!(reflex(&mut f, 0, &w.view(), &flora.view()).is_none());
    assert!(!f.drop_plans.contains_key(&id), "the jump is off");
    assert!(f.trapped.contains_key(&id), "the clock keeps running a moment off its feet");
}

#[test]
fn reach_scales_with_the_configured_body_height_and_rejects_nonfinite_reach() {
    let (_, _, f) = fixture();
    let p = f.config.founder(SP);
    let body = p.body_at(f.animals[0].body);
    let rule = DropRule {
        after_s: 0.0,
        max_m: 2.5,
        wound_per_m: 0.0,
        water_max_m: 0.0,
    };
    close(
        rule.reach_m(p, &body),
        rule.max_m * body.height_m / p.adult_height_m,
    );
    for max_m in [-1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(DropRule { max_m, ..rule }.reach_m(p, &body), 0.0);
    }
}

#[test]
fn taking_off_requires_forward_motion_and_facing_the_landing() {
    let (w, _, mut f) = fixture();
    let plan = arm(&mut f);
    take_off(
        &mut f,
        0,
        SP,
        &w.view(),
        Actions {
            forward: 0.0,
            ..go()
        },
    );
    assert_eq!(f.animals[0].site, plan.from);
    f.animals[0].pose.heading_rad = FRAC_PI_2;
    take_off(&mut f, 0, SP, &w.view(), go());
    assert_eq!(f.animals[0].site, plan.from);
    assert_eq!(f.ledger.drops_by_founder[SP.index()], 0);
    f.animals[0].pose.heading_rad = -FRAC_PI_2;
    take_off(&mut f, 0, SP, &w.view(), go());
    assert_eq!(f.animals[0].site, plan.land);
}

#[test]
fn a_landing_adds_to_an_existing_wound_and_is_booked_once() {
    let (w, flora, mut f) = fixture();
    let plan = arm(&mut f);
    let prior = 0.2;
    f.animals[0].wound = prior;
    let p = f.config.founder(SP);
    let height = f64::from(plan.from.y - plan.land.y) * V;
    let expected = p.drop.wound_per_m * (height - p.step_down_m);
    let before = f.view().organic();
    take_off(&mut f, 0, SP, &w.view(), go());
    close(f.animals[0].wound, prior + expected);
    close(f.ledger.drop_m_by_founder[SP.index()], height);
    close(f.ledger.drop_wound_by_founder[SP.index()], expected);
    take_off(&mut f, 0, SP, &w.view(), go());
    assert_eq!(f.ledger.drops_by_founder[SP.index()], 1);
    close(f.view().organic(), before);
    ledgers(&f, &flora);
}

#[test]
fn drop_wound_has_no_cost_at_or_below_the_configured_step() {
    let (_, _, f) = fixture();
    let p = f.config.founder(SP);
    let r = DropRule {
        after_s: 0.0,
        max_m: 4.0,
        wound_per_m: 0.7,
        water_max_m: 0.0,
    };
    for h in [0.0, p.step_down_m * 0.5, p.step_down_m] {
        assert_eq!(r.wound(p, h), 0.0);
    }
    close(r.wound(p, p.step_down_m + V), r.wound_per_m * V);
}

#[test]
fn accumulated_fall_wound_kills_as_fell_and_conserves_all_currencies() {
    let (w, mut flora, mut f) = fixture();
    let plan = arm(&mut f);
    f.animals[0].wound = 0.75;
    let excess = f64::from(plan.from.y - plan.land.y) * V - f.config.founder(SP).step_down_m;
    f.config.founders[SP.index()].drop.wound_per_m = 0.25 / excess;
    let id = f.animals[0].id;
    take_off(&mut f, 0, SP, &w.view(), go());
    assert_eq!(f.animals[0].wound, 1.0);
    f.step(&w, &mut flora);
    assert!(f.view().animal(id).is_none());
    assert_eq!(f.ledger.departed_founder(SP, Departure::Fell), 1);
    assert_eq!(f.ledger.departed_founder(SP, Departure::Starved), 0);
    ledgers(&f, &flora);
}

#[test]
fn calm_watch_does_not_block_a_trapped_preys_due_drop() {
    let (w, flora, mut f) = fixture();
    let id = f.animals[0].id;
    f.config.founders[SP.index()].vigilance.watch_calm = true;
    f.config.founders[SP.index()].vigilance.calm_margin_m = 0.0;
    let a = f.animals[0];
    let mut p = crate::Percept {
        n: 1,
        ..Default::default()
    };
    p.tracks[0] = crate::Track {
        known: true,
        id: 97,
        class: crate::perception::class::THREAT,
        modes: crate::perception::SIGHT,
        posture: crate::perception::posture::ASLEEP,
        x: a.pose.x + 1.0,
        z: a.pose.z,
        y_m: a.pose.y_m,
        ..Default::default()
    };
    f.percepts.insert(id, p);
    assert!(crate::vigil::respond(&mut f, 0, &w.view(), &flora.view(), go()).is_none());
    assert!(!f.percepts[&id].vigil.fleeing);
    reflex(&mut f, 0, &w.view(), &flora.view());
    f.tick = (f.config.founder(SP).drop.after_s / DT).round() as u64;
    assert!(reflex(&mut f, 0, &w.view(), &flora.view()).is_some());
}

#[test]
// DROP's brief: "prefers the lowest drop that reaches its route graph", the smallest fall
// (least wound), not the lowest landing. TEST-PASS-3 F1 read it the other way.
fn drop_prefers_the_smallest_fall_when_two_heights_reach_the_graph() {
    let (mut w, _, mut f) = fixture();
    // Open the floor south of the pocket, retaining both walls at the pocket itself.
    for x in 21..=23 {
        for z in 4..=7 {
            for y in 3..=11 {
                w.apply(WC::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Air,
                });
            }
        }
    }
    let a = f.animals[0];
    let p = *f.config.founder(SP);
    let b = p.body_at(a.body);
    let r = b.footprint_radius();
    let own = (r / V).ceil() as u32;
    let g = f.routes(&w.view(), SP, 0.0);
    let low = Site { x: 20, y: 2, z: 6 };
    assert!(
        g.node_near(
            low,
            ((f64::from(low.x) + 0.5) * V, (f64::from(low.z) + 0.5) * V),
            own
        )
        .is_some()
            || g.way_on(
                &w.view(),
                low,
                ((f64::from(low.x) + 0.5) * V, (f64::from(low.z) + 0.5) * V),
                r,
                b.headroom_voxels(V),
                p.step_limits(V),
                p.wade_depth_m(&b),
                own,
                8
            )
            .is_some(),
        "fixture: floor reaches the graph"
    );
    let way = g
        .drop_way(
            &w.view(),
            a.site,
            (a.pose.x, a.pose.z),
            r,
            b.headroom_voxels(V),
            p.step_limits(V),
            3,
            p.wade_depth_m(&b),
            own,
            8,
        )
        .expect("two reachable drops");
    // Excluding the two-layer jump with a larger step admits the very same three-layer
    // landing from the original pocket: the lower landing is a real drop candidate.
    let mut exclude_high = p.step_limits(V);
    exclude_high.down = 2;
    let lower = g
        .drop_way(
            &w.view(),
            a.site,
            (a.pose.x, a.pose.z),
            r,
            b.headroom_voxels(V),
            exclude_high,
            3,
            p.wade_depth_m(&b),
            own,
            8,
        )
        .expect("lower jump is clear");
    assert_eq!(lower.from, a.site);
    assert_eq!(lower.land.y, low.y);
    assert_eq!(way.from, a.site);
    assert!(
        way.land.y > low.y,
        "the smaller of two reachable falls: landed y={}, the deeper landing is y={}",
        way.land.y,
        low.y
    );
}

#[test]
fn a_drop_landing_on_a_moist_shore_face_is_an_eligible_egg_site() {
    let (mut w, flora, mut f) = fixture();
    let plan = arm(&mut f);
    // The neighbouring column is a shallow pond; the landing remains at its surface.
    w.apply(WC::SetMaterial {
        x: 18,
        y: 3,
        z: 8,
        material: Material::Air,
    });
    let cell = w.config().voxel_volume();
    w.apply(WC::AddWater {
        x: 18,
        y: 3,
        z: 8,
        volume_m3: cell,
    });
    w.apply(WC::SetMaterial {
        x: 19,
        y: 3,
        z: 8,
        material: Material::Air,
    });
    let cap = w.config().materials.pore_capacity(Material::Soil) * cell;
    w.apply(WC::AddWater {
        x: 19,
        y: 3,
        z: 8,
        volume_m3: 0.5 * cap,
    });
    w.apply(WC::SetMaterial {
        x: 19,
        y: 3,
        z: 8,
        material: Material::Soil,
    });
    let mut am = f.config.founder(Founder::RippleSnail).amphibious.unwrap();
    am.egg_water_max_m = V;
    am.egg_climb_max_m = 0.0;
    am.egg_moisture_min = 0.5;
    take_off(&mut f, 0, SP, &w.view(), go());
    assert_eq!(f.animals[0].site, plan.land);
    assert!(crate::shore_egg_site(&w.view(), &am, f.animals[0].site));
    ledgers(&f, &flora);
}
