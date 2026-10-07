//! Mean-field closures of the spatial rules; all ecological numbers come from sim config.
//! Well-mixed swept areas lose terrain barriers, known-kin tracks and learned places.
use super::laws::{self, Body};
use cubarium_voxel_fauna::{Amphibious, Food, FounderPhysiology, LightConfig, VigilMode};

#[derive(Clone, Copy, Debug, Default)]
pub struct ResponseState {
    pub hunker: f64,
    pub flight: f64,
}

/// Entering a threat disc is a swept-area flux 2 r v N/A. Leaving waits for
/// the threat to cross the outer disc and the condition-dependent calm (vigil.rs:664).
pub fn update_response(
    p: &FounderPhysiology,
    b: &Body,
    s: &mut ResponseState,
    density: f64,
    speed: f64,
    dt: f64,
) {
    if p.vigilance.mode != VigilMode::Herd {
        *s = ResponseState::default();
        return;
    }
    let h = p.hunker;
    let condition = (b.reserve / p.core.reserve_of(b.body).max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
    let can = h.enabled && condition >= h.min_condition;
    let (enter, leave, calm) = if can {
        (
            h.enter_m,
            h.leave_m,
            h.calm_s * condition.powf(h.hunger_calm_exp.max(0.0)),
        )
    } else {
        (
            p.vigilance.flee_start_m,
            p.vigilance.flee_stop_m,
            p.vigilance.calm_s,
        )
    };
    let on = 2.0 * enter.max(0.0) * speed.max(0.0) * density.max(0.0);
    let dwell = calm.max(0.0)
        + if speed > 0.0 {
            2.0 * leave.max(enter) / speed
        } else {
            0.0
        };
    let off = if dwell > 0.0 {
        1.0 / dwell
    } else {
        1.0 / dt.max(f64::MIN_POSITIVE)
    };
    let rate = on + off;
    let old = if can { s.hunker } else { s.flight };
    let share = (on / rate + (old - on / rate) * (-rate * dt).exp()).clamp(0.0, 1.0);
    *s = if can {
        ResponseState {
            hunker: share,
            flight: 0.0,
        }
    } else {
        ResponseState {
            hunker: 0.0,
            flight: share,
        }
    };
}

/// A renewal cycle: 1/net-work seconds shielded, opened_s seconds bared.
/// Resistance has the sim's growth exponent and capped kin ring (vigil.rs:222,918).
pub fn pry_open_share(p: &FounderPhysiology, body: f64, members: f64, kin: f64) -> f64 {
    let h = p.hunker;
    if !h.enabled {
        return 0.0;
    }
    let grown = (body / p.core.birth_body.max(f64::MIN_POSITIVE))
        .clamp(0.0, 1.0)
        .powf(h.shield_growth_exp.max(0.0));
    let resistance = h.resist_per_s
        * grown
        * (1.0 + h.ring_resist_per_kin * kin.clamp(0.0, f64::from(h.ring_kin_max)));
    let net = (members * h.pry_per_member_s - resistance).max(0.0);
    let open = net * h.opened_s.max(0.0);
    open / (1.0 + open)
}

/// Guard::Open/Shield/Bared (vigil.rs:895). Opened bodies lose armour too.
pub fn guarded_strike_share(
    h: &FounderPhysiology,
    hb: f64,
    p: &FounderPhysiology,
    pb: f64,
    hunker: f64,
    opened: f64,
) -> f64 {
    let normal = laws::strike_share(h, hb, p, pb);
    if !p.hunker.enabled {
        return normal;
    }
    let mut bare = *p;
    bare.armour = 0.0;
    let open = opened.clamp(0.0, 1.0);
    let shielded = normal * (1.0 - p.hunker.shield_of(pb, p.core.birth_body, 0.0));
    let guard = (1.0 - open) * shielded + open * laws::strike_share(h, hb, &bare, pb);
    let share = hunker.clamp(0.0, 1.0);
    (1.0 - share) * normal + share * guard
}

/// The existing model's profitability-squared kill law, averaged over guard time.
/// A bared interval must retain its own hazard; squaring the mean hit loses it.
pub fn guarded_kill_share(
    h: &FounderPhysiology,
    hb: f64,
    p: &FounderPhysiology,
    pb: f64,
    hunker: f64,
    opened: f64,
) -> f64 {
    let normal = laws::strike_share(h, hb, p, pb).powi(2);
    if !p.hunker.enabled {
        return normal;
    }
    let shield = guarded_strike_share(h, hb, p, pb, 1.0, 0.0).powi(2);
    let bare = guarded_strike_share(h, hb, p, pb, 1.0, 1.0).powi(2);
    let guard = hunker.clamp(0.0, 1.0);
    let open = opened.clamp(0.0, 1.0);
    (1.0 - guard) * normal + guard * ((1.0 - open) * shield + open * bare)
}

fn brightness(c: &LightConfig, p: &FounderPhysiology, day: f64) -> [f64; 3] {
    let uv = if c.learn { c.uv_innate } else { 1.0 };
    let cyan = if c.learn { c.cyan_innate } else { 1.0 };
    let flower = if p.senses.uv {
        c.flower_reflectance * day.max(0.0) * uv
    } else {
        0.0
    };
    let lure_expect = if p.senses.uv && c.lure_uv > 0.0 {
        uv
    } else {
        cyan
    };
    [
        flower.max(0.0),
        (c.glowcap_emission * cyan).max(0.0),
        ((c.lure_emission
            + if p.senses.uv {
                c.lure_uv * c.flower_reflectance
            } else {
                0.0
            })
            * lure_expect)
            .max(0.0),
    ]
}

fn speed(p: &FounderPhysiology, body: f64) -> f64 {
    p.cruise_body_lengths_per_s * p.body_at(body).length_m
}

fn visit_rate(c: &LightConfig, p: &FounderPhysiology, body: f64, count: f64, area: f64) -> f64 {
    if count <= 0.0 || area <= 0.0 || p.senses.light_m <= 0.0 {
        return 0.0;
    }
    // Encounter, approach to the beacon, then inspection; not every sighting is a visit.
    let v = speed(p, body);
    if v <= 0.0 {
        return 0.0;
    }
    let encounter = 2.0 * p.senses.light_m * v * count / area;
    let travel = p.senses.light_m / v;
    1.0 / (1.0 / encounter + travel + c.visit_s.max(0.0))
}

/// Lit hours enter as `lit`, awake hungry flyers as `awake`; flowers reflect by day,
/// glowcaps emit at night (light.rs:228,663). Daylight luring can plug into lit upstream.
pub fn lure_visits_per_s(
    c: &LightConfig,
    p: &FounderPhysiology,
    body: f64,
    awake: f64,
    lit: f64,
    flowers: f64,
    caps: f64,
    day: f64,
    area: f64,
) -> f64 {
    if awake <= 0.0 || lit <= 0.0 {
        return 0.0;
    }
    let b = brightness(c, p, day);
    if b[2] + c.lure_scent <= 0.0 {
        return 0.0;
    }
    let lure = lit * (b[2] + c.lure_scent.max(0.0));
    let competition = flowers.max(0.0) * b[0] + caps.max(0.0) * b[1];
    awake * visit_rate(c, p, body, lit, area) * lure / (lure + competition)
}

/// A flower encounter pays nectar per flower, bounded by the stomach's remaining capacity.
/// Travel/inspection limits access; digestion still limits the actual feeding in model.rs.
/// This replaces the old Hill-3 response to a dimensionful whole-world nectar stock.
pub fn nectar_response(
    c: &LightConfig,
    p: &FounderPhysiology,
    body: f64,
    nectar: f64,
    flowers: f64,
    caps: f64,
    day: f64,
    area: f64,
) -> f64 {
    if nectar <= 0.0 || flowers <= 0.0 {
        return 0.0;
    }
    let b = brightness(c, p, day);
    // This beacon closure lacks nocturnal nectar scent gradients.
    let flower_signal = b[0];
    if flower_signal <= 0.0 {
        return 0.0;
    }
    let success = flowers * flower_signal
        / (flowers * flower_signal + caps.max(0.0) * b[1]).max(f64::MIN_POSITIVE);
    let sip = (nectar / flowers).min(p.gut.capacity(body));
    let intake = visit_rate(c, p, body, flowers, area) * success * sip;
    // Search/inspection alternates with processing the sip. This is a renewal budget,
    // so longer failed inspections cannot manufacture a larger sip.
    let ceiling = laws::gut_ceiling_per_s(p, body, Food::Nectar);
    intake / (intake + ceiling).max(f64::MIN_POSITIVE)
}

/// Probability at least one reachable face beats the giving-up and per-snail worth
/// gates (step.rs:1694,1740). Edible zero triggers the ordinary empty-taste departure.
pub fn film_departure(
    am: &Amphibious,
    edible: f64,
    rich: f64,
    reachable: f64,
    neighbors: f64,
) -> f64 {
    if am.graze_search_m <= 0.0 || reachable <= 0.0 || (edible > 0.0 && edible >= am.graze_give_up)
    {
        return 0.0;
    }
    let per = rich / neighbors.max(1.0);
    if per < am.graze_worth || per <= edible {
        return 0.0;
    }
    1.0 - (-reachable / neighbors.max(1.0)).exp()
}

/// Shore rule (step.rs::egg_site): land, moist, within the horizontal and climb bands.
pub fn shore_eligible(
    am: &Amphibious,
    depth: f64,
    moisture: f64,
    distance: f64,
    height: f64,
) -> bool {
    depth <= 0.0
        && moisture >= am.egg_moisture_min
        && distance <= am.egg_water_max_m
        && height <= am.egg_climb_max_m
}

/// Consecutive exposure, strict > just as shore_eggs (step.rs:4145); losses go to litter.
pub fn egg_loss(
    am: &Amphibious,
    wet: &mut f64,
    dry: &mut f64,
    under: bool,
    moisture: f64,
    dt: f64,
) -> Option<bool> {
    *wet = if under { *wet + dt } else { 0.0 };
    *dry = if !under && moisture < am.egg_dry_moisture {
        *dry + dt
    } else {
        0.0
    };
    if *wet > am.egg_drown_s {
        Some(true)
    } else if *dry > am.egg_dry_s {
        Some(false)
    } else {
        None
    }
}

/// Habitat census carried alongside stock totals; counts are faces or beacon stands.
#[derive(Clone, Debug, Default)]
pub struct Habitat {
    pub area: f64,
    pub sky: f64,
    pub flowers: f64,
    pub glowcaps: f64,
    pub film_faces: f64,
    pub film_reference: f64,
    pub film_refuge: f64,
    pub film_rich: f64,
    pub film_land_share: f64,
    pub film_growth: f64,
    pub shore: Vec<ShoreBand>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ShoreBand {
    pub faces: f64,
    pub depth: f64,
    pub moisture: f64,
    pub distance: f64,
    pub height: f64,
}
