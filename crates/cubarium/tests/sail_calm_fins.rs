//! The sail's calm rest and feed fins (`art/creatures/sail.tscn`, reviewed in
//! `design/7_Research/fable-sail-calm-review-2026-09-13.md`): the shipped creature atlas must
//! carry exactly the point-baked sail rows the review adopted, the rest loop must hold its
//! fins still except for one late adjustment, the feed loop must hold its fins braced while
//! the body chews, and the move and bud rows must be the pre-existing ones. The fixture is
//! the 256×64 sail block (rest, move, feed, bud) of the current-source bake, which equalled
//! the study's brace candidate byte for byte.
use std::path::Path;

use cubarium::art::ArtPack;

const TILE: usize = 16;
const FRAMES: usize = 16;
/// Atlas rows of the sail's clips: rest, move, feed, bud.
const SAIL_ROWS: [usize; 4] = [4, 5, 6, 7];

fn atelier() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")
}

fn rgba(path: &Path) -> (usize, usize, Vec<u8>) {
    let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap())).read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba, "{}", path.display());
    buf.truncate(info.buffer_size());
    (info.width as usize, info.height as usize, buf)
}

fn tile(atlas: &(usize, usize, Vec<u8>), row: usize, frame: usize) -> Vec<[u8; 4]> {
    let (w, _, px) = atlas;
    (0..TILE)
        .flat_map(|y| (0..TILE).map(move |x| ((row * TILE + y) * w + frame * TILE + x) * 4))
        .map(|i| [px[i], px[i + 1], px[i + 2], px[i + 3]])
        .collect()
}

fn differing(a: &[[u8; 4]], b: &[[u8; 4]]) -> usize {
    a.iter().zip(b).filter(|(p, q)| p != q).count()
}

#[test]
fn the_shipped_sail_block_is_the_reviewed_bake_byte_for_byte() {
    let atlas = rgba(&atelier().join("creatures.png"));
    assert_eq!((atlas.0, atlas.1), (256, 256));
    let fixture = rgba(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/sail-calm-rows.png"));
    assert_eq!((fixture.0, fixture.1), (256, 64));
    for (k, row) in SAIL_ROWS.into_iter().enumerate() {
        for frame in 0..FRAMES {
            assert_eq!(tile(&atlas, row, frame), tile(&fixture, k, frame), "sail row {row} frame {frame}");
        }
    }
}

#[test]
fn rest_holds_its_fins_still_except_one_late_symmetric_adjustment() {
    // Rest keys: fins at 0 rad from 0 to 2.5 s, 0.2 rad at 3 s, back to 0 at 3.5 s, over a
    // 4 s loop sampled at 16 frames (0.25 s each). Point-baked, the half-way 0.1 rad poses
    // (frames 11 and 13) land on the held texels, so the whole adjustment is frame 12: the
    // presenter's blend between bracketing frames fades it in over 0.25 s and out over
    // 0.25 s, once per 4 s loop. Measured on the adopted bake: 21 fin texels at the peak.
    let atlas = rgba(&atelier().join("creatures.png"));
    let rest: Vec<Vec<[u8; 4]>> = (0..FRAMES).map(|f| tile(&atlas, 4, f)).collect();
    for f in (0..FRAMES).filter(|&f| f != 12) {
        assert_eq!(rest[f], rest[0], "rest frame {f} must be the held pose");
    }
    let peak = differing(&rest[12], &rest[0]);
    assert!((8..=24).contains(&peak), "one visible adjustment of a few fin texels: {peak}");
}

#[test]
fn feed_keeps_its_fins_braced_while_the_body_chews() {
    // Feed keys hold both fins at ±0.2 rad for the whole 2 s loop; only the body's chew
    // moves, at its 1 s period, so frame f equals frame f + 8 and the frames that differ
    // from frame 0 differ by the chew alone (never more texels than the chew's body pixels).
    let atlas = rgba(&atelier().join("creatures.png"));
    let feed: Vec<Vec<[u8; 4]>> = (0..FRAMES).map(|f| tile(&atlas, 6, f)).collect();
    for f in 0..8 {
        assert_eq!(feed[f], feed[f + 8], "feed frame {f} repeats at {}", f + 8);
    }
    // Every non-still frame changes the same texels: one chew, nothing else moving. On
    // the adopted bake that set is 23 body texels; a fin flick would add texels outside it
    // and vary the set from frame to frame.
    let changed_set = |f: usize| -> Vec<usize> { feed[f].iter().zip(&feed[0]).enumerate().filter(|(_, (p, q))| p != q).map(|(i, _)| i).collect() };
    let chew = changed_set(1);
    assert!((8..=32).contains(&chew.len()), "the body still chews, by a few texels: {}", chew.len());
    for f in 1..8 {
        assert_eq!(changed_set(f), chew, "feed frame {f} must move only the chew texels");
    }
    // The rest adjustment's fin texels and the chew's body texels are disjoint sets on
    // this art, as measured; recorded so a future bake that lets the fins move while
    // feeding is caught here.
    let rest: Vec<Vec<[u8; 4]>> = (0..FRAMES).map(|f| tile(&atlas, 4, f)).collect();
    let fins: Vec<usize> = rest[12].iter().zip(&rest[0]).enumerate().filter(|(_, (p, q))| p != q).map(|(i, _)| i).collect();
    assert!(fins.iter().all(|i| !chew.contains(i)), "chew texels overlap the fin adjustment texels");
}

#[test]
fn the_pack_loads_with_sixteen_frame_sail_clips_and_keeps_every_selector() {
    let art = ArtPack::load(&atelier()).unwrap();
    for clip in &art.clips {
        assert_eq!(clip.frames.len(), FRAMES);
    }
    let meta: serde_json::Value = serde_json::from_slice(&std::fs::read(atelier().join("pack.json")).unwrap()).unwrap();
    assert_eq!(meta["creature_names"][1], "sail");
    let tall = meta["tall"].as_array().unwrap();
    let selector = |name: &str, part: &str, key: &str| -> Option<String> {
        tall.iter().find(|r| r["name"] == name && r["part"] == part).and_then(|r| r.get(key)).and_then(|v| v.as_str()).map(String::from)
    };
    assert_eq!(selector("vinecoil", "trunk", "vine_strips").as_deref(), Some("period4_endpoint_v1"));
    assert_eq!(selector("spiretree", "crown", "corner_cap_owner").as_deref(), Some("final_position_v1"));
    assert_eq!(selector("glasscane", "crown", "corner_cap_owner").as_deref(), Some("final_position_v1"));
    assert!(art.tall_plant("spiretree").unwrap().corner_cap_owner && art.tall_plant("glasscane").unwrap().corner_cap_owner);
}
