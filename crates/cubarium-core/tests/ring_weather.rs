//! FW-6: the cylinder embedding and the weather that rides on it, written from
//! `design/flat-world-plan-2026-09-16.md` §5a — including its instruction that this file
//! **measures and records the cap aspect at the rims rather than asserting it round**.

use cubarium_core::config::{HabitatConfig, WeatherConfig, WorldConfig};
use cubarium_core::habitat::{Habitat, Weather};
use cubarium_core::world::World;
use cubarium_surface::{CellId, Face, Scale, SurfacePoint, Topology};

const W: u16 = 320;
const H: u16 = 180;
const COLS: u16 = 80;
const ROWS: u16 = 45;
const SEED: u64 = 20260916;

fn ring() -> Topology {
    Topology::Ring { w: W, h: H }
}

fn front(u: f64, v: f64) -> SurfacePoint {
    SurfacePoint::new(Face::Front, u, v)
}

/// §5a's embedding, written out here: `θ = 2π·u/w`, `r = w / (2π·32·S)`,
/// `y_e = (h/2 − v) / (32·S)`, `embed = [r cos θ, y_e, r sin θ]`.
fn reference_embed(w: f64, h: f64, s: f64, u: f64, v: f64) -> [f64; 3] {
    let theta = std::f64::consts::TAU * u / w;
    let r = w / (std::f64::consts::TAU * 32.0 * s);
    let y = (h / 2.0 - v) / (32.0 * s);
    [r * theta.cos(), y, r * theta.sin()]
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn normalize(p: [f64; 3]) -> [f64; 3] {
    let n = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    [p[0] / n, p[1] / n, p[2] / n]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

// ---------------------------------------------------------------------------
// The embedding
// ---------------------------------------------------------------------------

#[test]
fn the_embedding_is_the_isotropic_cylinder_of_the_plan() {
    for (topo, s) in [(ring(), 1.0), (Topology::Ring { w: 640, h: 360 }, 2.0)] {
        let Topology::Ring { w, h } = topo else { unreachable!() };
        let scale = Scale::new(s);
        for (u, v) in
            [(0.0, 0.0), (0.5, 0.5), (80.0, 45.0), (f64::from(w) - 0.5, f64::from(h) - 0.5)]
        {
            let got = topo.embed(scale, &front(u, v));
            let want = reference_embed(f64::from(w), f64::from(h), s, u, v);
            for k in 0..3 {
                assert!(
                    (got[k] - want[k]).abs() <= 1e-12,
                    "{topo:?} S = {s} at ({u}, {v}) component {k}: {} vs {}",
                    got[k],
                    want[k]
                );
            }
        }
        // The radius is what makes arc length per pixel `1/(32·S)`: both rungs of §6's
        // ladder give the same cylinder, 1.59155 units across.
        let r = f64::from(w) / (std::f64::consts::TAU * 32.0 * s);
        assert!((r - 1.591_549_430_918_953_5).abs() <= 1e-12, "{topo:?}: r = {r}");
    }
}

/// Seamless by construction: `u = 0` and `u = w` are the same point in 3D, so the noise the
/// habitat samples has no seam to hide.
#[test]
fn the_embedding_is_seamless_across_the_wrap() {
    let topo = ring();
    for v in [0.5, 45.0, 90.0, 179.5] {
        let west = topo.embed(Scale::ONE, &front(0.0, v));
        for eps in [1e-3, 1e-6, 1e-9] {
            let east = topo.embed(Scale::ONE, &front(f64::from(W) - eps, v));
            assert!(
                dist(west, east) <= eps * 0.04,
                "v = {v}, ε = {eps}: the two sides of the wrap are {} apart",
                dist(west, east)
            );
        }
    }
    // And the cells either side of the wrap are exactly one cell apart in 3D, like any
    // other pair of neighbours.
    let habitat = Habitat::new(&HabitatConfig::default(), SEED, topo, Scale::ONE);
    let at = |cx: u16, cy: u16| {
        habitat.positions[CellId::new(topo, Scale::ONE, Face::Front, cx, cy).index()]
    };
    for cy in [0u16, 22, 44] {
        let across = dist(at(COLS - 1, cy), at(0, cy));
        let inside = dist(at(0, cy), at(1, cy));
        assert!(
            (across - inside).abs() <= 1e-12,
            "row {cy}: {across} across the wrap vs {inside} inside the chart"
        );
    }
}

/// One pixel is `1/(32·S)` embedded units on **both** axes, which is what makes the habitat
/// patches round.
#[test]
fn one_pixel_is_the_same_embedded_distance_on_both_axes() {
    let topo = ring();
    let unit = 1.0 / 32.0;
    for (u, v) in [(0.5, 0.5), (160.5, 90.5), (319.5, 179.5)] {
        let here = topo.embed(Scale::ONE, &front(u, v));
        let east = topo.embed(Scale::ONE, &front((u + 1.0) % f64::from(W), v));
        let down = topo.embed(Scale::ONE, &front(u, v - 1.0));
        // The horizontal step is a chord, so it is a hair shorter than the arc.
        assert!(
            (dist(here, east) - unit).abs() <= unit * 1e-3,
            "at ({u}, {v}) one pixel east is {}",
            dist(here, east)
        );
        assert!(
            (dist(here, down) - unit).abs() <= 1e-15,
            "at ({u}, {v}) one pixel up is {}",
            dist(here, down)
        );
    }
}

/// The habitat noise has no seam at the wrap: the biggest step between wrap-adjacent cells
/// is no bigger than the biggest step between ordinary neighbours.
#[test]
fn the_habitat_noise_is_seamless_across_the_wrap() {
    let topo = ring();
    let habitat = Habitat::new(&HabitatConfig::default(), SEED, topo, Scale::ONE);
    let light = |cx: u16, cy: u16| {
        habitat.light_base[CellId::new(topo, Scale::ONE, Face::Front, cx, cy).index()]
    };
    let mut worst_inside: f64 = 0.0;
    let mut worst_wrap: f64 = 0.0;
    for cy in 0..ROWS {
        for cx in 0..COLS - 1 {
            worst_inside = worst_inside.max((light(cx, cy) - light(cx + 1, cy)).abs());
        }
        worst_wrap = worst_wrap.max((light(COLS - 1, cy) - light(0, cy)).abs());
    }
    assert!(
        worst_wrap <= worst_inside + 1e-12,
        "the wrap steps by {worst_wrap}, the chart by at most {worst_inside}"
    );
}

/// §5a: `embed()` drives position, `height()` drives the bands, and on a ring they are
/// different functions; `normalize()` of a cylinder point maps height monotonically to
/// latitude, which is what lets the spherical blob model stay unchanged.
#[test]
fn normalize_maps_height_monotonically_to_latitude() {
    let topo = ring();
    let mut last = f64::INFINITY;
    for v in 0..=180 {
        let v = f64::from(v);
        let p = normalize(topo.embed(Scale::ONE, &front(160.5, v)));
        let latitude = p[1].asin();
        assert!(latitude < last, "latitude must fall as v rises (v = {v})");
        last = latitude;
        // Azimuth is preserved: the point stays on its own meridian.
        let azimuth = p[2].atan2(p[0]);
        let want = std::f64::consts::TAU * 160.5 / f64::from(W);
        assert!(
            (azimuth.rem_euclid(std::f64::consts::TAU) - want).abs() <= 1e-12,
            "azimuth at v = {v}"
        );
    }
}

/// §5a's number: `y_e ∈ [−2.8125, 2.8125]` with `r = 1.59155`, so the world spans `±60.50°`
/// of latitude.
#[test]
fn the_latitude_span_is_sixty_point_five_degrees() {
    let topo = ring();
    let top = normalize(topo.embed(Scale::ONE, &front(0.0, 0.0)))[1].asin().to_degrees();
    let bottom =
        normalize(topo.embed(Scale::ONE, &front(0.0, f64::from(H))))[1].asin().to_degrees();
    assert!((top - 60.50).abs() <= 0.01, "top rim latitude {top}");
    assert!((bottom + 60.50).abs() <= 0.01, "bottom rim latitude {bottom}");
    assert_eq!(topo.embed(Scale::ONE, &front(0.0, 0.0))[1], 2.8125, "y_e at the top rim");
    assert_eq!(topo.embed(Scale::ONE, &front(0.0, f64::from(H)))[1], -2.8125);
}

// ---------------------------------------------------------------------------
// Weather is unchanged
// ---------------------------------------------------------------------------

/// §5a: "the config is unchanged … `blobs_per_channel` stays **3** and `amplitude` is
/// unchanged".
#[test]
fn blobs_per_channel_is_three_and_the_cap_is_fifty_five_degrees() {
    let cfg = WeatherConfig::default();
    assert_eq!(cfg.blobs_per_channel, 3, "§5a's conclusion: the cube's defaults transfer");
    assert_eq!(cfg.blob_radius_deg, 55.0);
    assert_eq!(cfg.amplitude, 0.3);

    let weather = Weather::new(&cfg, SEED);
    assert_eq!(weather.light.len(), 3);
    assert_eq!(weather.moisture.len(), 3);
    for b in weather.light.iter().chain(weather.moisture.iter()) {
        assert!((dot(b.center, b.center) - 1.0).abs() <= 1e-12, "the centre is a unit direction");
        assert!((dot(b.axis, b.axis) - 1.0).abs() <= 1e-12, "the axis is a unit direction");
        assert!(dot(b.center, b.axis).abs() <= 1e-9, "the axis is perpendicular to the centre");
        assert!(b.rate > 0.0);
    }
}

/// The blob model takes no topology, so a ring world and a cube world from the same seed
/// have bit-identical weather — §5a's "RNG stream parity is trivially exact".
#[test]
fn the_weather_stream_is_identical_on_a_ring_and_on_a_cube() {
    let mut cube_cfg = WorldConfig { seed: SEED, ..WorldConfig::default() };
    cube_cfg.topology = Topology::Cube;
    let mut ring_cfg = WorldConfig { seed: SEED, ..WorldConfig::default() };
    ring_cfg.topology = ring();

    let mut cube = World::new(cube_cfg).expect("legal cube world");
    let mut ring_world = World::new(ring_cfg).expect("legal ring world");
    assert_eq!(cube.state.weather, ring_world.state.weather, "weather at tick 0");

    // Far enough for the orbit and for the per-minute random walk (20 Hz × 60 = 1,200).
    for _ in 0..1_300 {
        cube.step();
        ring_world.step();
    }
    assert_eq!(
        cube.state.weather, ring_world.state.weather,
        "weather after 1,300 ticks, including the per-minute walk"
    );
    assert_eq!(cube.state.weather.last_walk_minute, 1, "the walk really happened");
    assert_ne!(
        Weather::new(&WeatherConfig::default(), SEED),
        cube.state.weather,
        "and the blobs really moved away from their initial state"
    );
}

/// The measurement §5a asks for, **recorded rather than asserted round**: the pixel-space
/// footprint of the default 55° cap, at the equator and centred on each rim row.
///
/// The rule is the model's own: a cell is in the cap when
/// `dot(blob.center, normalize(position)) >= cos(radius)`.
#[test]
fn the_cap_aspect_at_the_rims_is_measured_and_recorded() {
    let topo = ring();
    let habitat = Habitat::new(&HabitatConfig::default(), SEED, topo, Scale::ONE);
    let cos_radius = 55.0f64.to_radians().cos();
    let dir_of = |cx: u16, cy: u16| {
        normalize(habitat.positions[CellId::new(topo, Scale::ONE, Face::Front, cx, cy).index()])
    };

    let mut recorded = Vec::new();
    for (label, centre_row) in [("top rim", 0u16), ("equator", ROWS / 2), ("bottom rim", ROWS - 1)]
    {
        let centre = dir_of(COLS / 2, centre_row);
        let mut in_cap = vec![false; usize::from(COLS) * usize::from(ROWS)];
        for cy in 0..ROWS {
            for cx in 0..COLS {
                if dot(centre, dir_of(cx, cy)) >= cos_radius {
                    in_cap[usize::from(cy) * usize::from(COLS) + usize::from(cx)] = true;
                }
            }
        }
        // Half-width along the centre's own row, and the extent up and down its own column.
        let row = |cy: u16, cx: u16| in_cap[usize::from(cy) * usize::from(COLS) + usize::from(cx)];
        let half_width =
            (0..COLS / 2).take_while(|d| row(centre_row, (COLS / 2 + d) % COLS)).count();
        let mut down = 0usize;
        while centre_row + (down as u16) < ROWS && row(centre_row + down as u16, COLS / 2) {
            down += 1;
        }
        let mut up = 0usize;
        while (up as u16) <= centre_row && row(centre_row - up as u16, COLS / 2) {
            up += 1;
        }
        let cells = in_cap.iter().filter(|b| **b).count();
        let aspect = (up + down - 1) as f64 / (2 * half_width - 1).max(1) as f64;
        recorded.push((label, half_width, up, down, cells, aspect));
    }

    for (label, half_width, up, down, cells, aspect) in &recorded {
        println!(
            "{label}: half-width {half_width} cells, {up} up, {down} down, \
             {cells} of 3600 cells in the cap, height/width aspect {aspect:.3}"
        );
    }

    // What is asserted is only what the measurement can support, not "round":
    let equator = recorded.iter().find(|r| r.0 == "equator").unwrap();
    let top = recorded.iter().find(|r| r.0 == "top rim").unwrap();
    let bottom = recorded.iter().find(|r| r.0 == "bottom rim").unwrap();
    assert!(equator.4 > 0 && top.4 > 0 && bottom.4 > 0, "every cap covers some of the world");
    assert_eq!(top.4, bottom.4, "the two rims are mirror images: {} vs {}", top.4, bottom.4);
    assert!(
        (top.5 - bottom.5).abs() <= 1e-12,
        "and their aspects match: {} vs {}",
        top.5,
        bottom.5
    );
    assert!(
        (equator.5 - top.5).abs() > 0.1,
        "the cap is *not* the same shape at the rim as at the equator: {} vs {}",
        equator.5,
        top.5
    );
    // The equator cap is unclipped, so its half-width is the 55° azimuth arc: 55/360 of 80
    // cells = 12.2, and the ±55° meridian reaches r·tan(55°)·32 px = 18.2 cells.
    assert_eq!(equator.1, 13, "equator half-width in cells");
    assert!(equator.2 >= 18 && equator.2 <= 19, "equator reach up: {}", equator.2);
}
