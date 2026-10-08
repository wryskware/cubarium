//! Headless shader mechanisms on hand-built cells; no tuned defaults or world hashes.
#[path = "../examples/figure_capture/fixture.rs"]
mod fixture;
use fixture::{Cell, config, draw};

fn cell(offset: [i8; 3]) -> Cell {
    Cell {
        offset,
        colours: [[0.2, 0.15, 0.1]; 3],
    }
}

fn rgb(picture: &fixture::Picture, x: i32, y: i32) -> &[u8] {
    let at = ((y as usize * picture.projection.raster_w as usize + x as usize) * 4) as usize;
    &picture.pixels[at..at + 3]
}

#[test]
fn face_rows_use_the_atlas_outline_and_open_rim() {
    let mut cfg = config(0.0);
    cfg.light.sun = [0.0, 1.0, 0.0];
    let body = draw(&[cell([0, 0, 0])], 2, 0, false, &cfg).unwrap();
    let (x, y, w, h) = body.projection.front_rect(16, 4, 8);
    let (x, y) = (x as i32 + w as i32 / 2, y);
    assert!(
        rgb(&body, x, y + h as i32 - 1)[0] < rgb(&body, x, y + 2)[0],
        "outline darkens the bottom row"
    );
    // Make the cap light differ from the side, so a rim is observable.
    let model = draw(&[cell([0, 0, 0])], 0, 0, false, &cfg).unwrap();
    let covered = draw(&[cell([0, 0, 0]), cell([0, 1, 0])], 0, 0, false, &cfg).unwrap();
    assert!(
        rgb(&model, x, y)[0] > rgb(&model, x, y + 2)[0],
        "open rim leans toward the lit cap"
    );
    let s = cfg.px_per_voxel;
    use cubarium::voxel::appearance as a;
    assert_eq!(
        a::front_texel(5, a::GLYPH_BODY, 2, s - 1, s).tone(),
        a::TONE_OUTLINE
    );
    assert_eq!(
        a::front_texel(5, a::GLYPH_INTERIM, 2, 0, s).tone(),
        a::TONE_OPEN_RIM
    );
    assert_eq!(
        a::front_texel(5, a::GLYPH_INTERIM, 2, s - 1, s).tone(),
        a::TONE_PLAIN
    );
    assert_eq!(rgb(&model, x, y + h as i32 - 1), rgb(&model, x, y + 2));
    assert_eq!(rgb(&covered, x, y), rgb(&covered, x, y + 2));
}

#[test]
fn a_crease_is_only_the_band_beside_an_occluding_model_neighbour() {
    let cells = [cell([0, 0, 0]), cell([-1, 0, -1])];
    let cfg = config(0.7);
    let shaded = draw(&cells, 0, 0, false, &cfg).unwrap();
    let plain = draw(&cells, 0, 0, false, &config(0.0)).unwrap();
    let (x, y, w, h) = shaded.projection.front_rect(16, 4, 8);
    let b = ((cfg.px_per_voxel + 4) / 8).max(1).min(h / 2);
    for dy in 1..h - 1 {
        for dx in 0..w {
            let (a, p) = (
                rgb(&shaded, x as i32 + dx as i32, y + dy as i32),
                rgb(&plain, x as i32 + dx as i32, y + dy as i32),
            );
            assert_eq!(a != p, dx < b, "face texel {dx},{dy}");
        }
    }
    let coplanar = [cell([0, 0, 0]), cell([-1, 0, 0])];
    let a = draw(&coplanar, 0, 0, false, &cfg).unwrap();
    let b = draw(&coplanar, 0, 0, false, &config(0.0)).unwrap();
    assert_eq!(a.pixels, b.pixels, "coplanar cells do not create a crease");
}

#[test]
fn quarter_headings_match_the_stamp_with_model_neighbours() {
    let cells = [
        cell([0, 0, 0]),
        Cell {
            offset: [-1, 0, -1],
            colours: [[0.08, 0.12, 0.22]; 3],
        },
        Cell {
            offset: [0, 1, 1],
            colours: [[0.22, 0.06, 0.12]; 3],
        },
    ];
    for scale in [4, 7, 8] {
        let mut cfg = config(0.65);
        cfg.px_per_voxel = scale;
        for quarter in 0..4 {
            let stamp = draw(&cells, 0, quarter, true, &cfg).unwrap();
            let mut models = Vec::new();
            for supersample in [1, 2] {
                cfg.figures.supersample = supersample;
                let model = draw(&cells, 0, quarter, false, &cfg).unwrap();
                let worst = stamp.pixels.iter().zip(&model.pixels)
                    .map(|(a, b)| a.abs_diff(*b)).max().unwrap();
                assert!(worst <= 1,
                    "{scale} px/voxel, S={supersample}, quarter {quarter}: worst RGB code delta {worst}");
                models.push(model.pixels);
            }
            let changed = models[0].iter().zip(&models[1]).filter(|(a, b)| a != b).count();
            let worst = models[0].iter().zip(&models[1]).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            assert!(changed == 0,
                "{scale} px/voxel, quarter {quarter}: aligned SSAA blocks change {changed} codes, max {worst}");
        }
    }
}

#[test]
fn rotated_silhouettes_and_edging_gain_partial_coverage_at_whole_pixels() {
    // Equal pigments, no sun/AO/haze: every opaque sample has one colour.
    let cells = [Cell {
        offset: [0, 0, 0],
        colours: [[0.2; 3]; 3],
    }];
    let mut cfg = config(0.0);
    cfg.figures.supersample = 1;
    let hard = fixture::draw_angle(&cells, 0, 0, std::f64::consts::FRAC_PI_8, false, &cfg).unwrap();
    cfg.figures.supersample = 2;
    let aa = fixture::draw_angle(&cells, 0, 0, std::f64::consts::FRAC_PI_8, false, &cfg).unwrap();
    let opaque = hard.pixels.chunks_exact(4).map(|p| p[0]).max().unwrap();
    assert!(opaque > 0);
    assert!(
        hard.pixels
            .chunks_exact(4)
            .all(|p| p[0] == 0 || p[0] == opaque),
        "the S=1 silhouette has binary coverage"
    );
    assert!(
        aa.pixels.chunks_exact(4).any(|p| p[0] > 0 && p[0] < opaque),
        "S=2 shades subpixel geometry and downsamples partial silhouette coverage"
    );
    cfg.figures.supersample = 1;
    let hard_edged = fixture::draw_angle(&cells, 2, 0, std::f64::consts::FRAC_PI_8, false, &cfg).unwrap();
    cfg.figures.supersample = 2;
    let aa_edged = fixture::draw_angle(&cells, 2, 0, std::f64::consts::FRAC_PI_8, false, &cfg).unwrap();
    let interior_codes: std::collections::BTreeSet<_> = hard_edged.pixels.chunks_exact(4)
        .zip(aa.pixels.chunks_exact(4)).filter(|(_, p)| p[0] == opaque)
        .map(|(p, _)| p[0]).collect();
    assert!(aa_edged.pixels.chunks_exact(4).zip(aa.pixels.chunks_exact(4))
        .any(|(edge, plain)| plain[0] == opaque && !interior_codes.contains(&edge[0])),
        "fully covered pixels gain intermediate face-tone values: internal edging is AA too");
}
