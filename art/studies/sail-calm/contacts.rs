//! Read-only reduction of both actual-world runs; no recoloring or resampling.
use serde_json::{Value, json};
use std::{fs, io::BufReader, path::Path};
fn read(path: &Path) -> Vec<u8> {
    let mut r = png::Decoder::new(BufReader::new(fs::File::open(path).unwrap()))
        .read_info()
        .unwrap();
    let mut data = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut data).unwrap();
    assert_eq!(
        (info.width, info.height, info.color_type),
        (256, 128, png::ColorType::Rgb)
    );
    data.truncate(info.buffer_size());
    data
}
fn write(path: &Path, w: usize, h: usize, data: &[u8], zoom: usize) {
    let mut out = vec![0; w * h * 3 * zoom * zoom];
    for y in 0..h * zoom {
        for x in 0..w * zoom {
            out[(y * w * zoom + x) * 3..][..3]
                .copy_from_slice(&data[((y / zoom) * w + x / zoom) * 3..][..3]);
        }
    }
    let mut e = png::Encoder::new(
        fs::File::create(path).unwrap(),
        (w * zoom) as u32,
        (h * zoom) as u32,
    );
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(&out).unwrap();
}
fn main() {
    let a: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(a.len(), 3, "BRACE_RUN SETTLE_RUN NEW_OUTPUT");
    let b = Path::new(&a[0]);
    let s = Path::new(&a[1]);
    let out = Path::new(&a[2]);
    fs::create_dir(out).expect("new output only");
    let observations = fs::read(b.join("sails.json")).unwrap();
    assert_eq!(observations, fs::read(s.join("sails.json")).unwrap());
    let rows: Value = serde_json::from_slice(&observations).unwrap();
    let rows = rows.as_array().unwrap();
    let first = rows
        .iter()
        .find(|r| {
            r["elapsed"].as_u64().unwrap() >= 600
                && r["meal"]["weight_prev"] == 0.
                && r["meal"]["weight"].as_f64().unwrap_or(0.) > 0.
        })
        .unwrap();
    let elapsed = first["elapsed"].as_i64().unwrap();
    let reports: Vec<Value> = [b, s]
        .iter()
        .map(|p| serde_json::from_slice(&fs::read(p.join("report.json")).unwrap()).unwrap())
        .collect();
    for key in [
        "opening_hash",
        "closing_hash",
        "receipt",
        "frame_pairs",
        "actual_fed_sail_member_ticks",
    ] {
        assert_eq!(reports[0][key], reports[1][key], "{key}");
    }
    let mut checked = 0;
    for frame in 1770..=2702 {
        let name = format!("frame_{frame:05}.png");
        assert_eq!(
            fs::read(b.join("old").join(&name)).unwrap(),
            fs::read(s.join("old").join(name)).unwrap()
        );
        checked += 1;
    }
    let offsets = [-1, 0, 1, 3, 6, 9, 15, 30];
    let mut crops = vec![0; 24 * 8 * 24 * 3 * 3];
    let mut full = vec![0; 256 * 128 * 3 * 3];
    let mut selected = Vec::new();
    for (col, offset) in offsets.iter().enumerate() {
        let at = elapsed + offset;
        let r = rows
            .iter()
            .find(|r| {
                r["elapsed"] == at
                    && r["slot"] == first["slot"]
                    && r["generation"] == first["generation"]
            })
            .unwrap();
        // Presentation-only tracking crop; never used to position the actual organism.
        let face = r["face"].as_u64().unwrap();
        let (ox, oy) = match face {
            0 => (64, 64),
            1 => (128, 64),
            2 => (192, 64),
            3 => (0, 64),
            4 => (64, 0),
            _ => panic!(),
        };
        let cx = ox + r["u"].as_f64().unwrap().floor() as i32;
        let cy = oy + r["v"].as_f64().unwrap().floor() as i32;
        let name = format!("frame_{:05}.png", at * 3 + 2);
        for (row, path) in [b.join("old"), b.join("new"), s.join("new")]
            .iter()
            .enumerate()
        {
            let rgb = read(&path.join(&name));
            for y in 0..24 {
                for x in 0..24 {
                    let sx = cx + x as i32 - 12;
                    let sy = cy + y as i32 - 12;
                    if (0..256).contains(&sx) && (0..128).contains(&sy) {
                        let dst = ((row * 24 + y) * 192 + col * 24 + x) * 3;
                        crops[dst..dst + 3]
                            .copy_from_slice(&rgb[(sy as usize * 256 + sx as usize) * 3..][..3]);
                    }
                }
            }
            if *offset == 15 {
                for y in 0..128 {
                    full[(y * 768 + row * 256) * 3..][..256 * 3]
                        .copy_from_slice(&rgb[y * 256 * 3..][..256 * 3]);
                }
            }
        }
        selected.push(json!({"offset_ticks":offset,"observation":r,"fraction":2./3.,"png":name}));
    }
    write(&out.join("meal-crops-native.png"), 192, 72, &crops, 1);
    write(&out.join("meal-crops-6x.png"), 192, 72, &crops, 6);
    write(&out.join("crowded-native.png"), 768, 128, &full, 1);
    write(&out.join("crowded-2x.png"), 768, 128, &full, 2);
    fs::write(out.join("data.js"), format!("window.STUDY={};", json!({"brace":format!("../{}",b.file_name().unwrap().to_str().unwrap()),"settle":format!("../{}",s.file_name().unwrap().to_str().unwrap()),"first":first,"rows":rows}))).unwrap();
    fs::write(out.join("viewer.html"), include_str!("meal-viewer.html")).unwrap();
    fs::write(out.join("selection.json"),serde_json::to_vec_pretty(&json!({"order":["original","brace","settle"],"selector":"first actual sail meal onset at/after care boundary; not proof that intake came from care","first":first,"selected":selected,"original_encoded_frames_identical":checked,"observations_and_receipts_identical":true,"reports":reports})).unwrap()).unwrap();
}
