//! The three sensor/transport checks the R1a result left as "structurally true but untested".
//!
//! Structural truth is not tested truth: the sampler only *looks* chart-free because every
//! offset it receives has already been unfolded into the observer's chart, and save/resume
//! agreement cannot catch a sensor that is wrong in both runs. These measure the world's own
//! observation through `World::neural_observation`, which runs the same sampler the controller
//! does, from the same neighbour lists and rings, without stepping.

use cubarium_core::config::FounderKind;
use cubarium_core::genome::{Genome, decode};
use cubarium_core::ids::OrganismId;
use cubarium_core::neural::Policy;
use cubarium_core::neural::gru::Gru32;
use cubarium_core::neural::obs::{BODY, FOOD_FAR, FOOD_NEAR, SECTORS};
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::{DT, World, WorldConfig};
use cubarium_surface::{Face, SurfacePoint, Vec2, cell_of, travel};

/// An empty, weather-free world: nothing moves or grows that the fixture did not put there.
fn bare() -> World {
    let mut c = WorldConfig::default();
    c.founders.kinds.clear();
    c.founders.count = 0;
    c.weather.amplitude = 0.0;
    c.water.rain_rate = 0.0;
    c.producer.growth = 0.0;
    c.producer.mortality = 0.0;
    c.detritus.decomposition = 0.0;
    c.detritus.fall = 0.0;
    World::new(c).expect("an empty world is valid")
}

fn place(world: &mut World, pos: SurfacePoint, heading: Vec2, size: f32) -> OrganismId {
    let cfg = world.config().clone();
    let mut genome = Genome::founder(0.5, &cfg.drives);
    genome.size = size;
    genome.speed = 1.0;
    genome.clamp();
    let mut phenotype = decode(&genome, &cfg.organism);
    phenotype.sense_radius = 8.0;
    let id = world.state.organisms.insert(Organism {
        pos: pos.canonicalize(),
        heading: heading.normalized().expect("a heading"),
        ou: Vec2::ZERO,
        structure: phenotype.structure_adult,
        reserve: 0.5 * phenotype.reserve_max,
        energy: 0.9 * phenotype.energy_max,
        born_tick: world.tick(),
        hunger_memory: 0.0,
        mode: Mode::Resting,
        escrow: None,
        births: 0,
        genome,
        phenotype,
        parent: None,
        origin: Origin::Founder,
        turn_counter: Counter::default(),
        fed_this_tick: false,
    });
    let o = world.state.organisms.get(id).expect("placed");
    world.state.external_material_in += o.structure + o.reserve;
    id
}

/// Body-frame offset to a chart vector for this heading (`+y` is the clockwise side).
fn body_offset(heading: Vec2, forward: f64, side: f64) -> Vec2 {
    let h = heading.normalized().expect("a heading");
    let clockwise = Vec2::new(-h.y, h.x);
    h * forward + clockwise * side
}

/// The eight body-relative directions the fixture paints food in, at two ranges. Distinct
/// stocks per direction, so a sector that picked up the wrong cell reads a different number.
fn painted_offsets() -> Vec<(f64, f64, f64)> {
    let mut out = Vec::new();
    for (i, radius) in [4.5f64, 9.0].into_iter().enumerate() {
        for k in 0..8 {
            let a = std::f64::consts::TAU * f64::from(k) / 8.0;
            // A distinct stock per (range, direction), all well inside `P_max`.
            let stock = 0.08 + 0.05 * f64::from(k) + 0.45 * i as f64;
            out.push((radius * a.cos(), radius * a.sin(), stock));
        }
    }
    out
}

/// Paint the same body-relative food pattern around a body, wherever it happens to stand.
///
/// The offsets are travelled from the body exactly as any offset is travelled, so a seam in
/// the way is followed rather than ignored: the result is the *physically equivalent* layout,
/// which is the thing a seam is supposed to make no difference to.
fn paint(world: &mut World, id: OrganismId) {
    // Start from an empty larder. `Fields::new` seeds a habitat-dependent standing crop, so
    // two cells on two faces do not begin equal; leaving that in place would compare the
    // initial field rather than the sampler. The removal is booked so the material ledger
    // stays closed.
    let mut removed = 0.0;
    for cell in cubarium_surface::CellId::all() {
        let i = cell.index();
        removed += world.state.fields.p[i] + world.state.fields.f[i] + world.state.fields.d[i];
        world.state.fields.p[i] = 0.0;
        world.state.fields.f[i] = 0.0;
        world.state.fields.d[i] = 0.0;
        world.state.fields.de[i] = 0.0;
    }
    world.state.external_material_in -= removed;
    let (pos, heading) = {
        let o = world.state.organisms.get(id).expect("placed");
        (o.pos, o.heading)
    };
    for (forward, side, stock) in painted_offsets() {
        let there = travel(pos, body_offset(heading, forward, side)).end;
        world.state.fields.p[cell_of(&there).index()] = stock;
    }
}

/// **(a) Seam-equivalent sampling.** A body straddling a seam senses the same food and body
/// sector values as a physically equivalent flat layout: same stocks at the same body-relative
/// positions, same neighbour at the same body-relative position, body frame transported.
///
/// Habitat inputs are excluded per contract §9 — `light`, `height`, `up` and `water` are
/// genuinely different on a different face, and that difference is real sensory information,
/// not an error. What must agree is the 39 food channels and the 12 body channels.
#[test]
fn a_body_at_a_seam_senses_the_same_food_and_bodies_as_an_equivalent_flat_layout() {
    // Interior of the Top face, and in the last cell row before the Top/Front seam, with the
    // heading pointing straight at it so the near ring genuinely crosses.
    //
    // **Both stand at the exact centre of their cell** (`4·i + 2`). Where a body sits *within*
    // its cell decides which cells a fixed body-relative offset lands in, so two observers at
    // different sub-cell offsets would be painted two different patterns and the comparison
    // would measure the fixture rather than the seam.
    let flat = SurfacePoint::new(Face::Top, 34.0, 34.0);
    let seam = SurfacePoint::new(Face::Top, 34.0, 62.0);
    let heading = Vec2::new(0.0, 1.0);

    let mut sensed = Vec::new();
    for anchor in [flat, seam] {
        let mut world = bare();
        let id = place(&mut world, anchor, heading, 1.0);
        paint(&mut world, id);
        // A companion body two cells ahead, at the same body-relative offset in both worlds.
        let companion = travel(anchor, body_offset(heading, 6.0, 0.0)).end;
        place(&mut world, companion, heading, 1.0);
        // One step, so the pair pass has built this arrangement's neighbour lists.
        world.step();
        world.drain_events();
        let o = world.state.organisms.get(id).expect("alive");
        assert_eq!(o.pos.face, anchor.face, "the observer must not have wandered off");
        sensed.push(world.neural_observation(id).expect("an observation"));
    }

    let (flat_obs, seam_obs) = (sensed[0].0, sensed[1].0);
    let worst = |range: std::ops::Range<usize>| {
        range
            .map(|i| (i, (flat_obs[i] - seam_obs[i]).abs()))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .expect("a non-empty range")
    };

    // The own cell and the near ring — the cells the body is standing in and touching — agree
    // to the bit.
    let (near_i, near_d) = worst(0..FOOD_FAR);
    assert!(
        near_d < 1e-12,
        "own cell and near ring differ at channel {near_i} by {near_d:e}: \nflat {:?}\nseam {:?}",
        &flat_obs[FOOD_NEAR..FOOD_NEAR + 18],
        &seam_obs[FOOD_NEAR..FOOD_NEAR + 18],
    );

    // The far ring (hops 2–3) agrees to a few parts in a million. The residual is floating
    // point in the unfold, not a difference in what was sensed: a cell three hops away across
    // a chart boundary arrives at a bearing a few microradians off the flat case, which moves
    // a hair of weight between two overlapping sector windows. Nothing at that scale is
    // sensory information — the channel's own range is [0, 1] — but it is real, so the
    // tolerance says so instead of pretending the two are bit-identical.
    let (far_i, far_d) = worst(FOOD_FAR..FOOD_FAR + 18);
    assert!(
        far_d < 1e-5,
        "far ring differs at channel {far_i} by {far_d:e}, beyond unfold rounding"
    );

    // Bodies: presence and relative size, all 12 channels.
    let (body_i, body_d) = worst(BODY..BODY + 2 * SECTORS);
    assert!(
        body_d < 1e-9,
        "body channel {body_i} differs by {body_d:e}: flat {:.9}, seam {:.9}",
        flat_obs[body_i],
        seam_obs[body_i]
    );

    // The fixture has to be non-trivial: something was actually sensed in both.
    assert!(
        flat_obs[FOOD_NEAR..FOOD_NEAR + 18].iter().any(|x| *x > 0.0),
        "the near ring is empty, so this compared nothing"
    );
    assert!(
        flat_obs[FOOD_FAR..FOOD_FAR + 18].iter().any(|x| *x > 0.0),
        "the far ring is empty, so this compared nothing"
    );
    assert!(
        flat_obs[BODY..BODY + 2 * SECTORS].iter().any(|x| *x > 0.0),
        "the companion was not sensed, so the body channels compared nothing"
    );
    println!(
        "seam equivalence: near ring max |Δ| {near_d:e}, far ring {far_d:e}, bodies {body_d:e}"
    );
}

/// **(b) The 17th neighbour is not sensed.** `cfg.capacity.max_neighbors` is 16, and the list
/// is nearest-first, so the farthest body in range is simply absent from the observation. The
/// truncation is the world's existing sensing budget and is not disclosed to the policy; this
/// fixture says so out loud rather than leaving it to a reader.
#[test]
fn the_seventeenth_neighbour_in_range_is_not_sensed_and_the_fixture_says_so() {
    let mut world = bare();
    assert_eq!(world.config().capacity.max_neighbors, 16);
    let anchor = SurfacePoint::new(Face::Top, 32.0, 32.0);
    let heading = Vec2::new(1.0, 0.0);
    let id = place(&mut world, anchor, heading, 1.0);

    // Sixteen crowded close in behind the observer, and one alone dead ahead but further out.
    // Every one of them is inside `r_sense`, so all seventeen are *in range*.
    for k in 0..16 {
        let a = std::f64::consts::PI * (0.6 + 0.8 * f64::from(k) / 15.0);
        let spot = travel(anchor, body_offset(heading, 2.0 * a.cos(), 2.0 * a.sin())).end;
        place(&mut world, spot, heading, 0.6);
    }
    let far = travel(anchor, body_offset(heading, 6.5, 0.0)).end;
    let seventeenth = place(&mut world, far, heading, 0.6);

    world.step();
    world.drain_events();
    let obs = world.neural_observation(id).expect("an observation");

    // Sector 0 is centred on the heading, and only the seventeenth body lies that way.
    assert_eq!(
        obs.0[BODY], 0.0,
        "the 17th body in range was sensed: the bounded list should have dropped it"
    );
    // The sixteen behind it are sensed, so the fixture is measuring truncation and not silence.
    assert!(
        obs.0[BODY..BODY + 2 * SECTORS].iter().any(|x| *x > 0.0),
        "no body at all was sensed, so this proves nothing about truncation"
    );
    // And it really is in range: it is well inside the observer's sensing radius.
    let o = world.state.organisms.get(id).expect("alive");
    let reach = cubarium_surface::unfold(o.pos, world.state.organisms.get(seventeenth).expect("alive").pos, 32.0)
        .expect("the two are on one chart");
    assert!(
        reach.distance < o.phenotype.sense_radius,
        "the 17th body is {} px away against a sensing radius of {}",
        reach.distance,
        o.phenotype.sense_radius
    );
}

/// **(c) A held signed turn survives a seam.** `MotorRequest` takes a target orientation, so
/// the adapter rebuilds this tick's request by rotating the heading the body has *now*. A seam
/// changes the chart of that heading, and the request is rebuilt from the transported one, so
/// the physical turn per tick must be the same signed amount on both sides of the crossing.
#[test]
fn a_held_turn_turns_by_the_same_signed_amount_on_both_sides_of_a_seam() {
    let mut world = bare();
    // Heading at the Top/Front seam, moving toward it under thrust so the crossing happens.
    let id = place(
        &mut world,
        SurfacePoint::new(Face::Top, 34.0, 54.0),
        Vec2::new(0.0, 1.0),
        1.0,
    );
    let mut w = Gru32::zeros();
    w.b_o[0] = 8.0; // thrust ~1, to carry it over the seam
    // A gentle clockwise turn: enough to be well above the deadband and to measure, small
    // enough that the body still crosses instead of curving away from the seam first.
    w.b_o[1] = 0.12;
    w.b_o[2] = -8.0;
    w.b_o[3] = -8.0;
    w.b_o[4] = -8.0;
    world
        .attach_neural_policy(id, Policy::new(w))
        .expect("attach");

    let mut before_seam: Vec<f64> = Vec::new();
    let mut after_seam: Vec<f64> = Vec::new();
    let mut crossed = false;
    let mut face = world.state.organisms.get(id).expect("alive").pos.face;
    let mut heading = world.state.organisms.get(id).expect("alive").heading;
    for _ in 0..400 {
        world.step();
        world.drain_events();
        let o = world.state.organisms.get(id).expect("alive");
        let segments = world.moved_segments(id);
        let one_face = segments
            .first()
            .is_none_or(|f| segments.iter().all(|s| s.face == f.face));
        if !one_face || o.pos.face != face {
            // The crossing tick itself: the heading before and after are in different charts,
            // so their difference is a change of coordinates and measures nothing.
            crossed = true;
        } else {
            let d = (o.heading.screen_angle() - heading.screen_angle())
                .rem_euclid(std::f64::consts::TAU);
            let d = if d > std::f64::consts::PI {
                d - std::f64::consts::TAU
            } else {
                d
            };
            // Clockwise in the body frame is the negative screen angle.
            let turn = -d;
            if crossed {
                after_seam.push(turn);
            } else {
                before_seam.push(turn);
            }
        }
        face = o.pos.face;
        heading = o.heading;
        if after_seam.len() >= 5 {
            break;
        }
    }

    assert!(crossed, "the fixture never reached the seam");
    assert!(
        before_seam.len() >= 5 && after_seam.len() >= 5,
        "not enough ticks on both sides: {} before, {} after",
        before_seam.len(),
        after_seam.len()
    );
    let want = before_seam[before_seam.len() - 1];
    assert!(want > 0.0, "a positive `a₁` is a clockwise turn, measured {want}");
    for (i, turn) in after_seam.iter().enumerate() {
        assert!(
            (turn - want).abs() < 1e-9,
            "tick {i} after the seam turned {turn} against {want} before it; \
             the held rate did not survive the chart change"
        );
    }
    let _ = DT;
}
