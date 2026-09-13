//! Opt-in compatibility and rejection tests; no changes to production image pixels.
use super::*;
use std::{
    fs,
    io::{BufReader, BufWriter},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

fn atelier() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")
}
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    path: PathBuf,
    meta: serde_json::Value,
}
impl Fixture {
    fn new() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../captures/vine-loader-fixtures")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::create_dir(&path).unwrap();
        for entry in fs::read_dir(atelier()).unwrap() {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            fs::copy(entry.path(), path.join(entry.file_name())).unwrap();
        }
        let meta = serde_json::from_slice(&fs::read(path.join("pack.json")).unwrap()).unwrap();
        Self { path, meta }
    }
    fn vine(&mut self) -> &mut serde_json::Value {
        self.meta["tall"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["name"] == "vinecoil" && r["part"] == "trunk")
            .unwrap()
    }
    fn load(&self) -> Result<ArtPack> {
        fs::write(
            self.path.join("pack.json"),
            serde_json::to_vec(&self.meta).unwrap(),
        )
        .unwrap();
        ArtPack::load(&self.path)
    }
    fn reject(&self, reason: &str) {
        match self.load() {
            Ok(_) => panic!("accepted invalid {reason}"),
            Err(e) => assert!(
                format!("{e:#}").contains(reason),
                "wrong rejection: {e:#}; wanted{reason}"
            ),
        }
    }
    fn drift(&self, frame: usize, row: usize) {
        let path = self.path.join("tall.png");
        let mut reader = png::Decoder::new(BufReader::new(fs::File::open(&path).unwrap()))
            .read_info()
            .unwrap();
        let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut rgba).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        rgba.truncate(info.buffer_size());
        let offset = ((6 * 16 + row) * info.width as usize + frame * 16 + 7) * 4;
        rgba[offset] = rgba[offset].wrapping_add(1);
        rgba[offset + 3] = 255;
        let mut encoder = png::Encoder::new(
            BufWriter::new(fs::File::create(&path).unwrap()),
            info.width,
            info.height,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&rgba)
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}

#[test]
fn current_opt_in_preserves_original_texels_and_inherits_one_clock() {
    let flagged = ArtPack::load(&atelier()).unwrap();
    let mut fixture = Fixture::new();
    assert_eq!(fixture.vine()["vine_strips"], VINE_STRIPS_V1);
    fixture
        .vine()
        .as_object_mut()
        .unwrap()
        .remove("vine_strips");
    let legacy = fixture.load().unwrap();
    let a = flagged.tall_plant("vinecoil").unwrap();
    let b = legacy.tall_plant("vinecoil").unwrap();
    assert!(b.vine_strips.is_none());
    assert!(a.base.is_none() && a.crown.is_none() && a.cap.is_none());
    let strips = a.vine_strips.as_ref().unwrap();
    for clip in [&strips.trunk, &strips.endpoint] {
        assert_eq!(clip.frames.len(), a.trunk.frames.len());
        assert_eq!(clip.seconds, a.trunk.seconds);
        assert_eq!(clip.looping, a.trunk.looping);
    }
    for i in 0..a.trunk.frames.len() {
        for y in 0..16 {
            for x in 0..16 {
                let original = a.trunk.frames[i].texel(x, y);
                assert_eq!(original, b.trunk.frames[i].texel(x, y));
                assert_eq!(
                    strips.trunk.frames[i].texel(x, y),
                    if (1..15).contains(&y) {
                        original
                    } else {
                        [0.; 4]
                    }
                );
                assert_eq!(
                    strips.endpoint.frames[i].texel(x, y),
                    if (4..8).contains(&y) {
                        original
                    } else {
                        [0.; 4]
                    }
                );
            }
        }
    }
    for p in &flagged.tall {
        if p.name != "vinecoil" {
            assert!(p.vine_strips.is_none());
        }
    }
}

#[test]
fn malformed_selectors_and_unsupported_roles_are_errors_not_fallbacks() {
    for value in [
        serde_json::Value::Null,
        serde_json::json!(false),
        serde_json::json!(true),
        serde_json::json!(1),
        serde_json::json!({}),
        serde_json::json!("period4_endpoint_v2"),
    ] {
        let mut f = Fixture::new();
        f.vine()["vine_strips"] = value;
        f.reject("unsupported vine_strips selector");
    }
    for name in ["spiretree", "glasscane"] {
        let mut f = Fixture::new();
        f.meta["tall"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["name"] == name && r["part"] == "trunk")
            .unwrap()["vine_strips"] = serde_json::json!(VINE_STRIPS_V1);
        f.reject("only on the vinecoil trunk");
    }
    let mut f = Fixture::new();
    f.meta["tall"][0]["vine_strips"] = serde_json::json!(VINE_STRIPS_V1);
    f.reject("only on the vinecoil trunk");
    let mut f = Fixture::new();
    f.vine().as_object_mut().unwrap().remove("vine_strips");
    for row in f.meta["tall"].as_array_mut().unwrap() {
        if row["name"] == "vinecoil" {
            row["name"] = serde_json::json!("unflagged-other");
        } else if row["name"] == "spiretree" {
            row["name"] = serde_json::json!("vinecoil");
            if row["part"] == "trunk" {
                row["vine_strips"] = serde_json::json!(VINE_STRIPS_V1);
            }
        }
    }
    f.reject("no base or crown");
}

#[test]
fn every_original_row_and_frame_is_validated_before_clearing() {
    for frame in [0, 23] {
        for row in [0, 5, 15] {
            let f = Fixture::new();
            f.drift(frame, row);
            f.reject("four-row periodic");
            let mut legacy = Fixture::new();
            legacy.drift(frame, row);
            legacy.vine().as_object_mut().unwrap().remove("vine_strips");
            assert!(
                legacy
                    .load()
                    .unwrap()
                    .tall_plant("vinecoil")
                    .unwrap()
                    .vine_strips
                    .is_none(),
                "unflagged custom art acquired new restrictions"
            );
        }
    }
}

#[test]
fn geometry_and_clock_mismatches_refuse_opt_in() {
    for seconds in [
        serde_json::json!(0.),
        serde_json::json!(-1.),
        serde_json::Value::Null,
        serde_json::json!("3"),
    ] {
        let mut f = Fixture::new();
        let reason = if seconds.is_number() {
            "duration"
        } else {
            "seconds"
        };
        f.vine()["seconds"] = seconds;
        f.reject(reason);
    }
    for looping in [
        serde_json::json!(false),
        serde_json::Value::Null,
        serde_json::json!("true"),
    ] {
        let mut f = Fixture::new();
        f.vine()["loop"] = looping;
        f.reject("must loop");
    }
    let mut f = Fixture::new();
    f.vine()["loop"] = serde_json::json!(true);
    assert!(f.load().is_ok());
    let mut f = Fixture::new();
    f.meta["pivot"] = serde_json::json!([8, 7]);
    f.reject("unsupported art pack layout");
    let mut f = Fixture::new();
    f.meta["tile"] = serde_json::json!(15);
    f.reject("unsupported art pack layout");
    let mut f = Fixture::new();
    f.vine()["frames"] = serde_json::json!(23);
    f.reject("tall rows carry");
    let mut f = Fixture::new();
    f.meta["plant_frames"] = serde_json::json!(23);
    f.reject("atlas");
    // Exercise the derivation boundary itself, not only the outer pack-layout checks.
    let empty = |width, height, pivot| {
        Sprite::from_premultiplied(width, height, pivot, vec![[0.; 4]; width * height]).unwrap()
    };
    for (width, height, pivot) in [
        (15, 16, Vec2::new(8., 8.)),
        (16, 15, Vec2::new(8., 8.)),
        (16, 16, Vec2::new(7., 8.)),
    ] {
        let clip = Clip {
            frames: vec![empty(width, height, pivot); 2],
            seconds: 3.,
            looping: true,
        };
        assert!(derive_vine_strips(&clip).is_err());
    }
    for (seconds, looping, count) in [
        (f64::NAN, true, 2),
        (f64::INFINITY, true, 2),
        (3., false, 2),
        (3., true, 0),
        (3., true, 1),
    ] {
        let clip = Clip {
            frames: vec![empty(16, 16, Vec2::new(8., 8.)); count],
            seconds,
            looping,
        };
        assert!(derive_vine_strips(&clip).is_err());
    }
}
