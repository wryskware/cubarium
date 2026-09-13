//! Read-only mutation-site flow diagnostic for the paid juvenile bottleneck.
//!
//! Replays one retained arm of the corrected charging cohort from its own recorded
//! `post-initialization.cubw`, with the per-member flow ledger (`cubarium_core::flow`)
//! recording every reserve/energy/structure movement at the assignment that makes it.
//!
//! Nothing here changes ecology. The diagnostic earns the right to be believed with four
//! gates, all of which must pass before any number is printed as a result:
//!
//! 1. **Input identity** — the decoded opening's SHA256 and state hash equal the ones the
//!    original run recorded in `opening.json`.
//! 2. **Output identity** — the replayed closing state re-encodes to bytes identical to the
//!    retained `closing.cubw`, and its state hash equals `summary.json`'s. The build label is
//!    supplied to the encoder, not derived, so this is a statement about the state.
//! 3. **Observer neutrality** — the same replay is run twice, once with the ledger off and
//!    once on, and the two must agree on the closing bytes, the state hash and every record
//!    of both event streams. Enabling the observer is proven inert, not asserted to be.
//! 4. **Flow/stock reconciliation** — for every member and every tick of its life, the stocks
//!    the world actually holds must equal the previous tick's stocks plus exactly the flows
//!    recorded. Any mutation site the ledger missed shows up here as a residual.
//!
//! Usage:
//!   juvenile_flow <ARM_DIR> [--ticks N] [--out FILE]
//!
//! `ARM_DIR` is a retained arm, e.g.
//! `captures/hunter-charge-candidate-two-hour-512ee52/seed-8/specialist_on`.
//! `--ticks` runs a shorter prefix for a smoke check and explicitly drops gate 2.

use std::collections::BTreeMap;
use std::io::Write;
use std::process::{Command, Stdio};

use cubarium_core::flow::FlowLedger;
use cubarium_core::{World, WorldState, decode_snapshot, encode_snapshot, snapshot};
use serde_json::{Value, json};

const BUILD: &str = "0.1.0+512ee52";

/// The repository's existing convention for content hashes in an example: shell out rather
/// than pull a crypto dependency into this crate (`ambient_compare.rs`).
fn sha256(bytes: &[u8]) -> String {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(bytes)
        .expect("write");
    let out = child.wait_with_output().expect("sha256sum output");
    assert!(out.status.success(), "sha256sum failed");
    String::from_utf8(out.stdout)
        .expect("utf8")
        .split_whitespace()
        .next()
        .expect("digest")
        .to_string()
}

/// One replay. Returns the closing state, the whole recorded event stream and, when asked
/// for, the ledger. The two calls differ in exactly one argument.
///
/// The event stream is kept verbatim rather than digested: comparing the actual records is a
/// stronger neutrality check than comparing a hash of them, and it points at the first
/// divergence if there ever is one.
fn replay(
    opening: &WorldState,
    ticks: u64,
    record: bool,
) -> (WorldState, Vec<(u64, String)>, Option<FlowLedger>) {
    let mut world = World::from_state(opening.clone()).expect("retained opening is a valid world");
    if record {
        world.enable_flow_ledger();
    }
    let mut stream = Vec::new();
    for _ in 0..ticks {
        world.step();
        let life = world.drain_events();
        let hunter = world.drain_hunter_events();
        if !life.is_empty() || !hunter.is_empty() {
            stream.push((
                world.tick(),
                serde_json::to_string(&json!({"life": life, "hunter": hunter}))
                    .expect("events serialize"),
            ));
        }
    }
    let ledger = world.flow_ledger().cloned();
    (world.state, stream, ledger)
}

/// Per-member derived view. Everything here is arithmetic over recorded flows; no new
/// measurement and no attribution of a lumped debit to one of its terms.
fn member_view(m: &Value) -> Value {
    let f = |path: &[&str]| -> f64 {
        let mut cursor = m;
        for key in path {
            cursor = &cursor[key];
        }
        cursor.as_f64().unwrap_or(0.0)
    };
    let u = |path: &[&str]| -> u64 {
        let mut cursor = m;
        for key in path {
            cursor = &cursor[key];
        }
        cursor.as_u64().unwrap_or(0)
    };
    let reserve_in = f(&["digestion", "to_reserve"])
        + f(&["frugivory", "to_reserve"])
        + f(&["grazing", "to_reserve"])
        + f(&["scavenging", "to_reserve"]);
    let reserve_out = f(&["oxidation", "reserve_burned"])
        + f(&["growth", "reserve_spent"])
        + f(&["funding", "reserve_debit"]);
    let energy_in = f(&["digestion", "energy_gain"])
        + f(&["frugivory", "energy_gain"])
        + f(&["grazing", "energy_gain"])
        + f(&["scavenging", "energy_gain"])
        + f(&["oxidation", "energy_gained"]);
    let energy_out = f(&["upkeep", "paid"])
        + f(&["strike", "paid"])
        + f(&["handling", "paid"])
        + f(&["growth", "energy_cost"])
        + f(&["funding", "energy_debit"]);
    let observed = u(&["gate", "observed_ticks"]).max(1) as f64;
    json!({
        "reserve_in_total": reserve_in,
        "reserve_out_total": reserve_out,
        "reserve_net": reserve_in - reserve_out,
        "reserve_out_share_oxidation": f(&["oxidation", "reserve_burned"]) / reserve_out.max(f64::MIN_POSITIVE),
        "reserve_out_share_growth": f(&["growth", "reserve_spent"]) / reserve_out.max(f64::MIN_POSITIVE),
        "reserve_out_share_funding": f(&["funding", "reserve_debit"]) / reserve_out.max(f64::MIN_POSITIVE),
        "energy_in_total": energy_in,
        "energy_out_total": energy_out,
        "energy_net": energy_in - energy_out,
        "mean_pre_growth_reserve": f(&["gate", "reserve_sum"]) / observed,
        "mean_pre_growth_energy": f(&["gate", "energy_sum"]) / observed,
        "gate_open_fraction": u(&["gate", "branch_entered_ticks"]) as f64 / observed,
        "reserve_zero_fraction": u(&["gate", "reserve_zero_ticks"]) as f64 / observed,
        "oxidation_active_fraction": u(&["oxidation", "ticks"]) as f64 / observed,
        "oxidation_above_reference_fraction":
            u(&["oxidation", "above_reference_ticks"]) as f64 / observed,
        "upkeep_over_reserve_intake": f(&["upkeep", "paid"]) / reserve_in.max(f64::MIN_POSITIVE),
        "note": "shares are over this member's own recorded outflow; upkeep is an energy debit and is not a reserve outflow",
    })
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("ARM_DIR");
    let mut ticks: u64 = 144_000;
    let mut out: Option<String> = None;
    let mut full_horizon = true;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ticks" => {
                ticks = args.next().expect("--ticks N").parse().expect("tick count");
                full_horizon = false;
            }
            "--out" => out = Some(args.next().expect("--out FILE")),
            other => panic!("unknown argument {other}"),
        }
    }

    let opening_bytes = std::fs::read(format!("{dir}/post-initialization.cubw")).expect("opening");
    let opening_meta: Value = serde_json::from_slice(
        &std::fs::read(format!("{dir}/opening.json")).expect("opening.json"),
    )
    .expect("opening.json parses");
    let summary: Value = serde_json::from_slice(
        &std::fs::read(format!("{dir}/summary.json")).expect("summary.json"),
    )
    .expect("summary.json parses");

    // Gate 1: the input is the exact artifact the original run wrote.
    let opening_sha = sha256(&opening_bytes);
    assert_eq!(
        opening_sha,
        opening_meta["post_snapshot_sha256"]
            .as_str()
            .expect("recorded opening sha"),
        "the opening snapshot is not the retained one"
    );
    let (_, opening_state) = decode_snapshot(&opening_bytes).expect("opening decodes normally");
    assert_eq!(
        snapshot::state_hash(&opening_state).to_string(),
        opening_meta["post_state_hash"]
            .as_str()
            .expect("recorded opening state hash"),
        "the opening state hash is not the retained one"
    );
    let opening_tick = opening_state.tick;

    // Gate 3, first half: the identical replay with no observer at all.
    let (plain_state, plain_stream, none) = replay(&opening_state, ticks, false);
    assert!(none.is_none(), "no ledger was requested");
    let plain_bytes = encode_snapshot(&plain_state, BUILD);

    // …and with the observer recording.
    let (observed_state, observed_stream, ledger) = replay(&opening_state, ticks, true);
    let observed_bytes = encode_snapshot(&observed_state, BUILD);
    let ledger = ledger.expect("the ledger was requested");

    // Gate 3, second half.
    assert_eq!(
        plain_bytes, observed_bytes,
        "the observer changed the closing state"
    );
    assert_eq!(
        snapshot::state_hash(&plain_state),
        snapshot::state_hash(&observed_state),
        "the observer changed the state hash"
    );
    assert_eq!(
        plain_stream.len(),
        observed_stream.len(),
        "the observer changed how many ticks emitted events"
    );
    for (a, b) in plain_stream.iter().zip(observed_stream.iter()) {
        assert_eq!(a, b, "the observer changed an event record at tick {}", a.0);
    }

    // Gate 2: the replay reproduces the retained closing artifact exactly.
    let closing_state_hash = snapshot::state_hash(&observed_state).to_string();
    let closing_identity = if full_horizon {
        let closing_bytes = std::fs::read(format!("{dir}/closing.cubw")).expect("closing");
        assert_eq!(
            observed_bytes, closing_bytes,
            "the replayed closing state differs from the retained bytes"
        );
        assert_eq!(
            closing_state_hash,
            summary["closing_state_hash"]
                .as_str()
                .expect("recorded closing state hash"),
            "the replayed closing state hash differs from the retained one"
        );
        json!({
            "checked": true,
            "closing_snapshot_sha256": sha256(&observed_bytes),
            "recorded_closing_snapshot_sha256": summary["closing_snapshot_sha256"],
            "closing_state_hash": closing_state_hash,
        })
    } else {
        json!({
            "checked": false,
            "reason": format!("--ticks {ticks} is a prefix, not the recorded horizon; closing identity is not asserted"),
            "prefix_state_hash": closing_state_hash,
        })
    };

    // Gate 4.
    let violations = ledger.total_violations();
    let ledger_value = serde_json::to_value(&ledger).expect("ledger serializes");
    let mut derived = BTreeMap::new();
    if let Some(members) = ledger_value["members"].as_array() {
        for m in members {
            let key = format!("{}:{}", m["id"]["slot"], m["id"]["generation"]);
            derived.insert(key, member_view(m));
        }
    }
    assert_eq!(
        violations, 0,
        "flow/stock reconciliation failed: a mutation site is missing from the ledger"
    );

    let report = json!({
        "kind": "juvenile-mutation-site-flow-diagnostic",
        "arm": dir,
        "build_label_supplied_to_encoder": BUILD,
        "opening": {
            "tick": opening_tick,
            "snapshot_sha256": opening_sha,
            "state_hash": snapshot::state_hash(&opening_state).to_string(),
            "seed": opening_state.config.seed,
            "arm": opening_meta["arm"],
            "profile_recipe": opening_meta["profile_recipe"],
            "world_oxidation_threshold": opening_meta["world_oxidation_threshold"],
            "member_oxidation_threshold": opening_meta["world_member_oxidation_threshold"],
        },
        "replayed_ticks": ticks,
        "closing_identity": closing_identity,
        "observer_neutrality": {
            "checked": true,
            "closing_bytes_equal": true,
            "state_hash_equal": true,
            "event_ticks_compared": plain_stream.len(),
            "event_stream_sha256": sha256(
                plain_stream
                    .iter()
                    .map(|(t, s)| format!("{t} {s}\n"))
                    .collect::<String>()
                    .as_bytes()
            ),
            "basis": "two full replays from the same decoded opening, differing only in enable_flow_ledger()",
        },
        "reconciliation": {
            "violations": violations,
            "tolerance": cubarium_core::flow::RESIDUAL_TOLERANCE,
            "basis": "per member per tick: stocks(t) - stocks(t-1) == sum of flows recorded at the mutation sites in tick t; the death tick closes against the removal site",
        },
        "ledger": ledger_value,
        "derived": derived,
        "limits": "Measures where a member's own reserve and energy moved. It does not establish a counterfactual: no juvenile in this cohort ran under the world's configured oxidation threshold, so the ledger cannot attribute the deficit to the raised one. Upkeep is recorded as three demanded terms and one clamped payment; the payment is deliberately not split across them.",
    });
    let text = serde_json::to_string_pretty(&report).expect("report serializes");
    match out {
        Some(path) => {
            if let Some(parent) = std::path::Path::new(&path).parent() {
                std::fs::create_dir_all(parent).expect("report directory");
            }
            std::fs::write(&path, format!("{text}\n")).expect("write report");
            eprintln!("all four gates passed; ledger written to {path}");
        }
        None => println!("{text}"),
    }
}
