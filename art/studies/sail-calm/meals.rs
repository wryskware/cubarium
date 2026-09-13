//! Frozen3147775 host: two point-baked art packs, one actual world and meal controller.
use cubarium::{
    art::ArtPack,
    art_present::{ArtPresenter, rig_of},
    sink::png::write_net_png,
};
use cubarium_core::{
    World,
    care::{CareCommand, CareKind, CareTarget},
    decode_snapshot,
};
use cubarium_render::Canvas;
use cube_proto::Frame;
use serde_json::json;
use std::{fs, path::PathBuf};

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        (4..=5).contains(&args.len()),
        "WORLD ORIGINAL_PACK CANDIDATE_PACK NEW_OUTPUT [CANDIDATE_NAME]"
    );
    let candidate = args.get(4).map(String::as_str).unwrap_or("brace");
    assert!(["brace", "settle"].contains(&candidate));
    let out = PathBuf::from(&args[3]);
    fs::create_dir(&out).expect("new output directory");
    for mode in ["old", "new"] {
        fs::create_dir(out.join(mode)).unwrap();
    }
    let (meta, state) = decode_snapshot(&fs::read(&args[0]).unwrap()).unwrap();
    let mut world = World::from_state(state).unwrap();
    let opening_hash = cubarium_core::snapshot::state_hash(&world.state);
    let mut old = ArtPresenter::new(ArtPack::load(&PathBuf::from(&args[1])).unwrap());
    let mut new = ArtPresenter::new(ArtPack::load(&PathBuf::from(&args[2])).unwrap());
    let mut receipt = json!(null);
    let mut evidence = Vec::new();
    let mut frame_pairs = 0;
    let mut fed_sail_ticks = 0;
    let mut canvas = Canvas::new();
    let mut frame = Frame::black();
    for elapsed in 0..1000u64 {
        if elapsed == 600 {
            receipt = serde_json::to_value(world.apply_care(&CareCommand::standard(
                world.care().admitted_seq + 1,
                world.tick(),
                CareKind::Feed,
                CareTarget {
                    face: 0,
                    u: 32.,
                    v: 48.,
                },
            )))
            .unwrap();
        }
        world.step();
        world.drain_events();
        let events = world.drain_hunter_events();
        let view = world.render_view();
        let hunters = world.hunter_view();
        for presenter in [&mut old, &mut new] {
            presenter.observe(&view);
            presenter.observe_hunters(&view, &hunters, &events).unwrap();
        }
        for o in &view.organisms {
            if rig_of(o.form, o.hue, 4) == 1 && o.fed {
                fed_sail_ticks += 1;
            }
            if (590..=900).contains(&elapsed) && rig_of(o.form, o.hue, 4) == 1 {
                let memory = new.meal_of(o.id).map(
                    |m| json!({"onset":m.onset,"weight":m.weight,"weight_prev":m.weight_prev}),
                );
                evidence.push(json!({"elapsed":elapsed,"tick":view.tick,"slot":o.id.slot,"generation":o.id.generation,"face":o.pos.face.index(),"u":o.pos.u,"v":o.pos.v,"fed":o.fed,"meal":memory,"juvenile":o.juvenile,"gestation":o.gestation}));
            }
        }
        if (590..=900).contains(&elapsed) {
            let hash = cubarium_core::snapshot::state_hash(&world.state);
            for k in 0..3u64 {
                for (name, presenter) in [("old", &mut old), ("new", &mut new)] {
                    canvas.clear();
                    presenter.draw(&view, k as f64 / 3., &mut canvas);
                    canvas.encode(&mut frame);
                    let mut rgb = Vec::new();
                    cubarium::net::net_rgb8(&frame, &mut rgb);
                    write_net_png(
                        &out.join(name)
                            .join(format!("frame_{:05}.png", elapsed * 3 + k)),
                        &rgb,
                    )
                    .unwrap();
                }
                frame_pairs += 1;
            }
            assert_eq!(
                hash,
                cubarium_core::snapshot::state_hash(&world.state),
                "drawing mutated ecology"
            );
        }
    }
    assert!(
        fed_sail_ticks > 0,
        "fixture must include actual sail intake"
    );
    fs::write(
        out.join("sails.json"),
        serde_json::to_vec(&evidence).unwrap(),
    )
    .unwrap();
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&json!({
        "candidate":candidate, "world":args[0],"original_pack":args[1],"candidate_pack":args[2],
        "input_schema":meta.schema,"opening_hash":format!("{opening_hash:016x}"),
        "closing_hash":format!("{:016x}",cubarium_core::snapshot::state_hash(&world.state)),
        "ticks":1000,"feed_at":600,"receipt":receipt,"frame_pairs":frame_pairs,
        "actual_fed_sail_member_ticks":fed_sail_ticks,"draw_ecology_unchanged":true,
        "note":"Same world trajectory and current real meal controller in both. fed means actual intake, not proof of manually supplied crumb origin. No creature form is forced."
    })).unwrap()).unwrap();
}
