//! Read-only replay of the retained seed-6 charging failure. Never repairs the state.
use cubarium_core::{World, decode_snapshot};

fn main() {
    let dir = std::env::args().nth(1).expect("retained arm directory");
    let initial = std::fs::read(format!("{dir}/post-initialization.cubw")).unwrap();
    let closing = std::fs::read(format!("{dir}/closing.cubw")).unwrap();
    let (_, state) = decode_snapshot(&initial).unwrap();
    assert_eq!(state.tick, 144000);
    assert_eq!(state.config.seed, 6);
    let mut world = World::from_state(state).unwrap();
    for _ in 0..26772 {
        let before = (world.tick() >= 170770).then(|| world.state.hunters.clone());
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            world.step();
        }));
        // Debug builds assert the invariant inside step; release harnesses report it in the
        // caller. In either case the completed invalid state must be kept, never repaired.
        if outcome.is_err() {
            assert_eq!(world.tick(), 170772, "unexpected replay panic");
        }
        let life = world.drain_events();
        let hunter = world.drain_hunter_events();
        if let Some(before) = before {
            println!(
                "{}",
                serde_json::json!({
                    "tick": world.tick(), "members_before": before.members,
                    "members_after": world.state.hunters.members,
                    "life": life, "hunter": hunter,
                    "validation": world.state.validate(),
                    "state_hash": cubarium_core::snapshot::state_hash(&world.state).to_string(),
                })
            );
        }
    }
    // Equal full serialized state, not merely the same validation message. Encoding retains
    // the failed state as evidence; ordinary decoding must still reject its semantic defect.
    assert_eq!(
        cubarium_core::encode_snapshot(&world.state, "0.1.0+3b06596"),
        closing
    );
    let error = decode_snapshot(&closing).unwrap_err();
    assert!(error.to_string().contains("stalking no one"));
    eprintln!("exact retained closing bytes reproduced; ordinary decoder refuses: {error}");
}
