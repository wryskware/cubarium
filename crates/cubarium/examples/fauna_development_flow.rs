//! Replay a retained pre-hunter seed with the transient per-member developmental flow
//! ledger attached, and emit one bounded JSON artifact describing every member's life.
//!
//! This is a **read-only developmental diagnostic**. It changes no configuration, no rate,
//! no threshold, no gate, no cost and no geometry. It replays a world that already exists
//! and writes down what the tick does to each member. There is no counterfactual anywhere
//! in this program: nothing was changed, so no number here attributes anything to any
//! mechanism.
//!
//! Four gates decide whether the artifact is worth reading at all. The run exits nonzero,
//! loudly, unless every one of them passes — and it writes the artifact first, because a
//! failing gate is a result to preserve, never something to repair.
//!
//! 1. **Input identity.** The tick-zero snapshot this replay starts from is byte-for-byte
//!    the retained one, by schema, build label, payload length, CRC32, whole-file SHA-256
//!    and FNV-1a-64 state hash.
//! 2. **Closing ecology identity.** Replaying to the horizon and re-encoding with the
//!    supplied build label reproduces the retained closing snapshot exactly. Both sides are
//!    schema 9; this is a same-schema comparison and is reported as one.
//! 3. **Observer neutrality.** The same world run twice, differing only in whether the
//!    ledger is attached, produces identical closing bytes, an identical event stream record
//!    for record, identical per-tick counters on every tick, and identical residuals.
//! 4. **Flow/stock reconciliation.** For every member and every tick of its life, the stocks
//!    the world actually holds equal the stocks it opened the tick with plus exactly the
//!    flows the ledger recorded, to 1e-12 absolute — with zero unregistered records, because
//!    a record arriving for a member the ledger never opened is a missed lifecycle edge.
//!
//! A fifth, separately-labelled check reconstructs the census from the per-individual
//! records alone and requires it to equal the retained telemetry at every sample. That is
//! what makes the per-member story an *explanation* of the observed trajectory rather than
//! something merely sitting beside it.

use anyhow::{Context, Result, bail};
use clap::Parser;
use cubarium_core::devflow::{DevFlowLedger, MemberRecord};
use cubarium_core::snapshot::{SCHEMA_VERSION, state_hash};
use cubarium_core::world::TickCounters;
use cubarium_core::{
    DT, LifeEvent, OrganismId, World, WorldConfig, WorldState, decode_snapshot, ecology_hash,
    encode_snapshot,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

/// The revision this worktree is based on. Supplied, not derived: see `notes`.
const BASE_REVISION: &str = "1d7b386";
/// The build label handed to the encoder so a re-encode is comparable with the retained
/// cohort. It is an input to this program, not a fact about the binary running it.
const BUILD_LABEL: &str = "0.1.0+1d7b386";
/// The retained cohort's telemetry cadence, and this replay's.
const TELEMETRY_EVERY: u64 = 100;
/// The reconciliation tolerance this diagnostic states, checked against the ledger's own.
const STATED_TOLERANCE: f64 = 1e-12;
/// Forms 0..=3 are the entire fauna at this revision. 4..=7 exist as slots and are always
/// zero here; they are reported as zeros, which is a null exposure and not an outcome.
const FORM_NAMES: [Option<&str>; 8] =
    [Some("grazer"), Some("glider"), Some("burrower"), Some("skimmer"), None, None, None, None];

#[derive(Parser)]
#[command(
    about = "Read-only per-member developmental flow replay of a retained pre-hunter seed."
)]
struct Args {
    /// Which retained seed to replay. Nothing else about the cohort is selected by this.
    #[arg(long)]
    seed: u64,
    /// Where to write the JSON artifact. Always written, including when a gate fails.
    #[arg(long)]
    out: PathBuf,
    /// The retained cohort directory holding `initial-manifest.json`, `manifest.json`,
    /// `initial-seeds/seed-N/world-0.cubw` and `seed-N/`.
    #[arg(
        long,
        default_value = "/home/wrysk/wryskware/cubarium/captures/hunter-openings-2026-09-13"
    )]
    cohort: PathBuf,
    /// Replay horizon. Short of the retained opening tick, gate 2 cannot be asserted and
    /// says so rather than passing.
    #[arg(long, default_value_t = 144000)]
    ticks: u64,
    /// Investigation lever: construct the world from the seed instead of loading the
    /// retained tick-zero snapshot. Gate 1 then becomes "the constructed tick-zero state
    /// re-encodes to the retained `world-0.cubw`".
    #[arg(long)]
    from_seed: bool,
}

// ---------------------------------------------------------------------------------------
// Hashes. Implemented here rather than pulled in as dependencies so this diagnostic adds
// nothing to the workspace's dependency graph or lockfile.
// ---------------------------------------------------------------------------------------

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// SHA-256 of a byte slice, lowercase hex. Pinned by `sha256_matches_known_vectors`.
fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = Vec::with_capacity(data.len() + 72);
    message.extend_from_slice(data);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());

    let mut w = [0u32; 64];
    for block in message.chunks_exact(64) {
        for (i, word) in w.iter_mut().take(16).enumerate() {
            *word = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ ((!v[4]) & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v[7] = v[6];
            v[6] = v[5];
            v[5] = v[4];
            v[4] = v[3].wrapping_add(t1);
            v[3] = v[2];
            v[2] = v[1];
            v[1] = v[0];
            v[0] = t1.wrapping_add(t2);
        }
        for (dst, add) in h.iter_mut().zip(v) {
            *dst = dst.wrapping_add(add);
        }
    }
    h.iter().map(|w| format!("{w:08x}")).collect()
}

/// FNV-1a 64, the same function `cubarium_core::snapshot` hashes payloads with. Used here
/// both to hash a snapshot payload straight off disk and to fold a rolling digest.
fn fnv1a(seed: u64, bytes: &[u8]) -> u64 {
    let mut h = seed;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn fold_u64(h: &mut u64, v: u64) {
    *h = fnv1a(*h, &v.to_le_bytes());
}

/// Every field of the tick's counters, folded in a fixed order. Floats are folded by their
/// bit pattern, so two runs that differ in the last bit of `heat_out` differ in the digest.
fn fold_counters(h: &mut u64, c: &TickCounters) {
    fold_u64(h, u64::from(c.births));
    for d in c.deaths {
        fold_u64(h, u64::from(d));
    }
    fold_u64(h, u64::from(c.cap_rejections));
    for v in [c.light_in, c.heat_out, c.rain_in, c.evap_out] {
        fold_u64(h, v.to_bits());
    }
    fold_u64(h, u64::from(c.travel_fallbacks));
    fold_u64(h, u64::from(c.travel_ties));
}

fn counters_json(c: &TickCounters) -> Value {
    json!({
        "births": c.births,
        "deaths": c.deaths,
        "cap_rejections": c.cap_rejections,
        "light_in": c.light_in,
        "heat_out": c.heat_out,
        "rain_in": c.rain_in,
        "evap_out": c.evap_out,
        "travel_fallbacks": c.travel_fallbacks,
        "travel_ties": c.travel_ties,
    })
}

/// u64 hashes are emitted as decimal strings: a JSON number cannot hold them without a
/// reader silently rounding through an f64.
fn u64s(v: u64) -> Value {
    Value::String(v.to_string())
}

fn id_json(id: OrganismId) -> Value {
    json!({ "slot": id.slot, "generation": id.generation })
}

// ---------------------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------------------

/// One replay's observable surface, recorded identically whether or not the ledger is on.
struct Run {
    closing_bytes: Vec<u8>,
    closing_state_hash: u64,
    closing_ecology_hash: u64,
    events: Vec<LifeEvent>,
    events_digest: u64,
    counters_digest: u64,
    final_counters: TickCounters,
    ticks_stepped: u64,
    telemetry: Vec<cubarium_core::Telemetry>,
    mass_residual: f64,
    water_residual: f64,
    population: usize,
    ledger: Option<Box<DevFlowLedger>>,
}

fn replay(mut world: World, horizon: u64, instrumented: bool) -> Run {
    if instrumented {
        world.enable_dev_flow_ledger();
    }
    let mut events = Vec::new();
    let mut events_digest = FNV_OFFSET;
    let mut counters_digest = FNV_OFFSET;
    let mut final_counters = TickCounters::default();
    let mut telemetry = Vec::new();
    let start = world.tick();
    while world.tick() < horizon {
        let counters = world.step().clone();
        fold_counters(&mut counters_digest, &counters);
        final_counters = counters;
        for event in world.drain_events() {
            let encoded = serde_json::to_vec(&event).expect("a life event is encodable");
            events_digest = fnv1a(events_digest, &encoded);
            events.push(event);
        }
        // Sampled on exactly the retained cohort's cadence, in both runs, so the counters
        // this resets are reset at the same ticks in both.
        if world.tick() % TELEMETRY_EVERY == 0 {
            telemetry.push(world.telemetry());
        }
    }
    let mass_residual = world.mass_residual();
    let water_residual = world.water_residual();
    let population = world.population();
    let closing_bytes = encode_snapshot(&world.state, BUILD_LABEL);
    let closing_state_hash = state_hash(&world.state);
    let closing_ecology_hash = ecology_hash(&world.state);
    let mut ledger = world.take_dev_flow_ledger();
    if let Some(l) = &mut ledger {
        l.finalize();
    }
    Run {
        closing_bytes,
        closing_state_hash,
        closing_ecology_hash,
        events,
        events_digest,
        counters_digest,
        final_counters,
        ticks_stepped: horizon - start,
        telemetry,
        mass_residual,
        water_residual,
        population,
        ledger,
    }
}

// ---------------------------------------------------------------------------------------
// Cohort inputs
// ---------------------------------------------------------------------------------------

fn manifest_entry<'a>(list: &'a [Value], seed: u64, key: &str) -> Option<&'a Value> {
    list.iter().find(|e| e.get(key).and_then(Value::as_u64) == Some(seed))
}

fn u(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(Value::as_u64)
}
/// A u64 the manifest records as a decimal string, for the same reason this artifact does.
fn hs(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(Value::as_str).and_then(|s| s.parse().ok())
}

/// A single compared value: what was computed, what was expected, and whether they agree.
fn cmp_str(label: &str, got: &str, want: Option<&str>) -> (Value, bool) {
    let ok = want == Some(got);
    (
        json!({ "field": label, "computed": got, "expected": want, "equal": ok }),
        ok,
    )
}
fn cmp_u64(label: &str, got: u64, want: Option<u64>) -> (Value, bool) {
    let ok = want == Some(got);
    (
        json!({
            "field": label,
            "computed": u64s(got),
            "expected": want.map(u64s),
            "equal": ok,
        }),
        ok,
    )
}

struct Checks {
    items: Vec<Value>,
    ok: bool,
}
impl Checks {
    fn new() -> Checks {
        Checks { items: Vec::new(), ok: true }
    }
    fn push(&mut self, (item, ok): (Value, bool)) {
        self.ok &= ok;
        self.items.push(item);
    }
    fn note(&mut self, item: Value) {
        self.items.push(item);
    }
}

// ---------------------------------------------------------------------------------------
// Census reconstructed from the per-individual records alone
// ---------------------------------------------------------------------------------------

/// A member is in the arena at sample tick `t` exactly when it had been placed by then and
/// its removal boundary has not arrived: the core removes a member during the step that
/// advances the clock to `death.event_tick`, so it is absent from tick `event_tick` onward.
fn alive_at(m: &MemberRecord, t: u64) -> bool {
    m.born_tick <= t && m.death.as_ref().is_none_or(|d| d.event_tick > t)
}

fn census_at(ledger: &DevFlowLedger, t: u64) -> ([u32; 8], u32) {
    let mut by_form = [0u32; 8];
    let mut total = 0u32;
    for m in ledger.members.values() {
        if alive_at(m, t) {
            by_form[(m.form as usize).min(7)] += 1;
            total += 1;
        }
    }
    (by_form, total)
}

// ---------------------------------------------------------------------------------------
// Per-member artifact rows
// ---------------------------------------------------------------------------------------

/// The founder ancestor and the number of parent hops to reach it. Derived in the harness,
/// not at the recording sites: it is a property of the whole set of records, not of one
/// mutation.
fn ancestry(
    members: &BTreeMap<OrganismId, MemberRecord>,
) -> BTreeMap<OrganismId, (OrganismId, u64)> {
    let mut out: BTreeMap<OrganismId, (OrganismId, u64)> = BTreeMap::new();
    for &id in members.keys() {
        let mut chain = Vec::new();
        let mut cursor = id;
        let root_depth = loop {
            if let Some(known) = out.get(&cursor) {
                break *known;
            }
            let Some(m) = members.get(&cursor) else {
                // A parent the ledger never opened would be an unregistered edge; the
                // chain stops at the last member that exists rather than inventing one.
                break (cursor, 0);
            };
            match m.parent {
                None => break (cursor, 0),
                Some(p) => {
                    chain.push(cursor);
                    cursor = p;
                }
            }
        };
        let (root, mut depth) = root_depth;
        out.entry(cursor).or_insert((root, depth));
        for who in chain.into_iter().rev() {
            depth += 1;
            out.insert(who, (root, depth));
        }
    }
    out
}

fn member_row(m: &MemberRecord, root: OrganismId, depth: u64) -> Result<Value> {
    let mut row = match serde_json::to_value(m)? {
        Value::Object(o) => o,
        other => bail!("a member record serialized as {other} rather than an object"),
    };
    row.insert(
        "form_name".into(),
        FORM_NAMES[(m.form as usize).min(7)].map_or(Value::Null, |n| Value::String(n.into())),
    );
    row.insert("root".into(), id_json(root));
    row.insert("lineage_depth".into(), Value::from(depth));
    // The removal interval, stated rather than implied. The ledger already carries both
    // endpoints; naming the difference here keeps the artifact readable without a reader
    // re-deriving the core's `now + 1` convention.
    if let Some(d) = &m.death {
        row.insert(
            "death_interval".into(),
            json!({
                "last_stepped_tick": d.last_stepped_tick,
                "decision_tick": d.decision_tick,
                "event_tick": d.event_tick,
                "ticks_between_last_step_and_event": d.ticks_between_last_step_and_event,
            }),
        );
    }
    Ok(Value::Object(row))
}

// ---------------------------------------------------------------------------------------
// Per-form aggregates
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct FormAgg {
    ever: u64,
    founders: u64,
    descendants: u64,
    reached_adult: u64,
    first_adult_tick: Option<u64>,
    last_adult_tick: Option<u64>,
    alive_at_close: u64,
    deaths: BTreeMap<String, u64>,
    stepped_ticks: u64,
    ticks_juvenile: u64,
    ticks_adult: u64,

    reserve_in_frugivory: f64,
    reserve_in_grazing: f64,
    reserve_in_scavenging: f64,
    reserve_in_refund: f64,
    reserve_out_oxidation: f64,
    reserve_out_growth: f64,
    reserve_out_reproduction: f64,

    energy_in_frugivory: f64,
    energy_in_grazing: f64,
    energy_in_scavenging: f64,
    energy_in_oxidation: f64,
    energy_in_refund: f64,
    energy_out_upkeep: f64,
    energy_out_growth: f64,
    energy_out_reproduction: f64,

    upkeep_demand_maintenance: f64,
    upkeep_demand_movement: f64,
    upkeep_demand_sensing: f64,
    upkeep_demand_total: f64,
    upkeep_paid_total: f64,
    upkeep_shortfall_total: f64,
    upkeep_ticks: u64,
    upkeep_ticks_underpaid: u64,

    gate_observations: u64,
    gate_structure_open: u64,
    gate_reserve_open: u64,
    gate_both_open: u64,
    gate_entered: u64,
    gate_positive_steps: u64,
    gate_ticks_reserve_zero: u64,
    /// `None` when no member of this form ever saw the reserve side closed at an
    /// observation — which is a null exposure, not a zero deficit.
    gate_min_deficit: Option<f64>,
    gate_max_closest_approach: Option<f64>,
    gate_bound_rate: u64,
    gate_bound_headroom: u64,
    gate_bound_reserve: u64,
    gate_bound_energy: u64,
    structure_gained: f64,

    request_ticks: u64,
    ticks_any_actual_intake: u64,
    contested_ticks: u64,
    headroom_limited_ticks: u64,
    requested_frugivory: f64,
    requested_grazing: f64,
    requested_scavenging: f64,
    actual_frugivory: f64,
    actual_grazing: f64,
    actual_scavenging: f64,

    cell_sampled_ticks: u64,
    cell_access_none: u64,
    cell_access_producer_only: u64,
    cell_access_detritus_only: u64,
    cell_access_both: u64,
    cell_producer_sum: f64,
    cell_fruit_sum: f64,
    cell_edible_detritus_sum: f64,
    cell_ticks_fruit_present: u64,
    cell_ticks_water_positive: u64,

    bud_decisions: u64,
    blocked_by_existing_escrow: u64,
    blocked_by_cap: u64,
    blocked_by_reserve: u64,
    blocked_by_energy: u64,
    funded: u64,
    births_delivered: u64,
    refunds: u64,
    miscarriages: u64,

    oxidation_ticks_fired: u64,
    oxidation_ticks_threshold_open: u64,
    oxidation_ticks_blocked_empty_reserve: u64,
}

impl FormAgg {
    fn observe(&mut self, m: &MemberRecord, horizon: u64) {
        self.ever += 1;
        if m.parent.is_none() {
            self.founders += 1;
        } else {
            self.descendants += 1;
        }
        if let Some(t) = m.structure.adult_recruitment_tick {
            self.reached_adult += 1;
            self.first_adult_tick = Some(self.first_adult_tick.map_or(t, |o: u64| o.min(t)));
            self.last_adult_tick = Some(self.last_adult_tick.map_or(t, |o: u64| o.max(t)));
        }
        if alive_at(m, horizon) {
            self.alive_at_close += 1;
        }
        if let Some(d) = &m.death {
            *self.deaths.entry(d.cause.to_string()).or_default() += 1;
        }
        self.stepped_ticks += m.reconciliation.checks;
        self.ticks_juvenile += m.structure.ticks_juvenile;
        self.ticks_adult += m.structure.ticks_adult;

        let i = &m.intake;
        self.reserve_in_frugivory += i.frugivory.to_reserve;
        self.reserve_in_grazing += i.grazing.to_reserve;
        self.reserve_in_scavenging += i.scavenging.to_reserve;
        self.reserve_in_refund += m.reproduction.refund_reserve;
        self.reserve_out_oxidation += m.oxidation.reserve_burned;
        self.reserve_out_growth += m.growth_gate.reserve_spent;
        self.reserve_out_reproduction += m.reproduction.reserve_debit;

        self.energy_in_frugivory += i.frugivory.energy;
        self.energy_in_grazing += i.grazing.energy;
        self.energy_in_scavenging += i.scavenging.energy;
        self.energy_in_oxidation += m.oxidation.energy_gained;
        self.energy_in_refund += m.reproduction.refund_energy;
        self.energy_out_upkeep += m.upkeep.paid_total;
        self.energy_out_growth += m.growth_gate.energy_spent;
        self.energy_out_reproduction += m.reproduction.energy_debit;

        self.upkeep_demand_maintenance += m.upkeep.demand_maintenance;
        self.upkeep_demand_movement += m.upkeep.demand_movement;
        self.upkeep_demand_sensing += m.upkeep.demand_sensing;
        self.upkeep_demand_total += m.upkeep.demand_total;
        self.upkeep_paid_total += m.upkeep.paid_total;
        self.upkeep_shortfall_total += m.upkeep.shortfall_total;
        self.upkeep_ticks += m.upkeep.ticks;
        self.upkeep_ticks_underpaid += m.upkeep.ticks_underpaid;

        let g = &m.growth_gate;
        self.gate_observations += g.observations;
        self.gate_structure_open += g.structure_side_open;
        self.gate_reserve_open += g.reserve_side_open;
        self.gate_both_open += g.both_open;
        self.gate_entered += g.entered;
        self.gate_positive_steps += g.positive_steps;
        self.gate_ticks_reserve_zero += g.ticks_reserve_zero;
        if g.closest_deficit.is_finite() {
            self.gate_min_deficit =
                Some(self.gate_min_deficit.map_or(g.closest_deficit, |o: f64| o.min(g.closest_deficit)));
            self.gate_max_closest_approach = Some(
                self.gate_max_closest_approach
                    .map_or(g.closest_approach_reserve, |o: f64| o.max(g.closest_approach_reserve)),
            );
        }
        self.gate_bound_rate += g.bound_by_rate;
        self.gate_bound_headroom += g.bound_by_headroom;
        self.gate_bound_reserve += g.bound_by_reserve;
        self.gate_bound_energy += g.bound_by_energy;
        self.structure_gained += g.structure_gained;

        self.request_ticks += i.request_ticks;
        self.ticks_any_actual_intake += i.ticks_any_actual_intake;
        self.contested_ticks += i.contested_ticks;
        self.headroom_limited_ticks += i.headroom_limited_ticks;
        self.requested_frugivory += i.frugivory.requested_amount;
        self.requested_grazing += i.grazing.requested_amount;
        self.requested_scavenging += i.scavenging.requested_amount;
        self.actual_frugivory += i.frugivory.actual_amount;
        self.actual_grazing += i.grazing.actual_amount;
        self.actual_scavenging += i.scavenging.actual_amount;

        let c = &m.local_cell;
        self.cell_sampled_ticks += c.sampled_ticks;
        self.cell_access_none += c.access_class_ticks.none;
        self.cell_access_producer_only += c.access_class_ticks.producer_only;
        self.cell_access_detritus_only += c.access_class_ticks.detritus_only;
        self.cell_access_both += c.access_class_ticks.both;
        self.cell_producer_sum += c.producer_sum;
        self.cell_fruit_sum += c.fruit_sum;
        self.cell_edible_detritus_sum += c.edible_detritus_sum;
        self.cell_ticks_fruit_present += c.ticks_fruit_present;
        self.cell_ticks_water_positive += c.ticks_water_positive;

        let r = &m.reproduction;
        self.bud_decisions += r.bud_decisions;
        self.blocked_by_existing_escrow += r.blocked_by_existing_escrow;
        self.blocked_by_cap += r.blocked_by_cap;
        self.blocked_by_reserve += r.blocked_by_reserve;
        self.blocked_by_energy += r.blocked_by_energy;
        self.funded += r.funded;
        self.births_delivered += r.births_delivered;
        self.refunds += r.refunds;
        self.miscarriages += r.miscarriages;

        self.oxidation_ticks_fired += m.oxidation.ticks_fired;
        self.oxidation_ticks_threshold_open += m.oxidation.ticks_threshold_open;
        self.oxidation_ticks_blocked_empty_reserve += m.oxidation.ticks_blocked_by_empty_reserve;
    }

    fn json(&self, index: usize) -> Value {
        json!({
            "form": index,
            "name": FORM_NAMES[index],
            "members_ever": self.ever,
            "founders": self.founders,
            "descendants": self.descendants,
            "reached_adult_target": self.reached_adult,
            "first_adult_recruitment_tick": self.first_adult_tick,
            "last_adult_recruitment_tick": self.last_adult_tick,
            "alive_at_horizon": self.alive_at_close,
            "deaths_by_cause": self.deaths,
            "stepped_ticks": self.stepped_ticks,
            "ticks_juvenile": self.ticks_juvenile,
            "ticks_adult": self.ticks_adult,
            "structure_gained": self.structure_gained,
            "reserve_inflow": {
                "frugivory_to_reserve": self.reserve_in_frugivory,
                "grazing_to_reserve": self.reserve_in_grazing,
                "scavenging_to_reserve": self.reserve_in_scavenging,
                "escrow_refund": self.reserve_in_refund,
            },
            "reserve_outflow": {
                "oxidation_burned": self.reserve_out_oxidation,
                "growth_spent": self.reserve_out_growth,
                "reproduction_debit": self.reserve_out_reproduction,
            },
            "energy_inflow": {
                "frugivory": self.energy_in_frugivory,
                "grazing": self.energy_in_grazing,
                "scavenging": self.energy_in_scavenging,
                "oxidation": self.energy_in_oxidation,
                "escrow_refund": self.energy_in_refund,
            },
            "energy_outflow": {
                "upkeep_paid": self.energy_out_upkeep,
                "growth_build_cost": self.energy_out_growth,
                "reproduction_debit": self.energy_out_reproduction,
            },
            "upkeep": {
                "demand_maintenance": self.upkeep_demand_maintenance,
                "demand_movement": self.upkeep_demand_movement,
                "demand_sensing": self.upkeep_demand_sensing,
                "demand_total": self.upkeep_demand_total,
                "paid_total": self.upkeep_paid_total,
                "shortfall_total": self.upkeep_shortfall_total,
                "ticks": self.upkeep_ticks,
                "ticks_underpaid": self.upkeep_ticks_underpaid,
                "note": "paid is a single clamped debit and is deliberately not split across the three demand terms",
            },
            "growth_gate": {
                "observations": self.gate_observations,
                "structure_side_open": self.gate_structure_open,
                "reserve_side_open": self.gate_reserve_open,
                "both_open": self.gate_both_open,
                "entered": self.gate_entered,
                "positive_steps": self.gate_positive_steps,
                "ticks_reserve_zero": self.gate_ticks_reserve_zero,
                "smallest_closest_deficit": self.gate_min_deficit,
                "largest_closest_approach_reserve": self.gate_max_closest_approach,
                "bound_by_rate": self.gate_bound_rate,
                "bound_by_headroom": self.gate_bound_headroom,
                "bound_by_reserve": self.gate_bound_reserve,
                "bound_by_energy": self.gate_bound_energy,
            },
            "intake": {
                "request_ticks": self.request_ticks,
                "ticks_any_actual_intake": self.ticks_any_actual_intake,
                "contested_ticks": self.contested_ticks,
                "headroom_limited_ticks": self.headroom_limited_ticks,
                "requested_potential": {
                    "frugivory": self.requested_frugivory,
                    "grazing": self.requested_grazing,
                    "scavenging": self.requested_scavenging,
                },
                "actual_transferred": {
                    "frugivory": self.actual_frugivory,
                    "grazing": self.actual_grazing,
                    "scavenging": self.actual_scavenging,
                },
            },
            "local_cell_potential_access": {
                "sampled_ticks": self.cell_sampled_ticks,
                "access_class_ticks": {
                    "none": self.cell_access_none,
                    "producer_only": self.cell_access_producer_only,
                    "detritus_only": self.cell_access_detritus_only,
                    "both": self.cell_access_both,
                },
                "producer_sum": self.cell_producer_sum,
                "fruit_sum": self.cell_fruit_sum,
                "edible_detritus_sum": self.cell_edible_detritus_sum,
                "ticks_fruit_present": self.cell_ticks_fruit_present,
                "ticks_water_positive": self.cell_ticks_water_positive,
                "note": "POTENTIAL access only: occupancy of a cell holding stock, never intake",
            },
            "reproduction": {
                "bud_decisions": self.bud_decisions,
                "blocked_by_existing_escrow": self.blocked_by_existing_escrow,
                "blocked_by_cap": self.blocked_by_cap,
                "blocked_by_reserve": self.blocked_by_reserve,
                "blocked_by_energy": self.blocked_by_energy,
                "funded": self.funded,
                "births_delivered": self.births_delivered,
                "refunds": self.refunds,
                "miscarriages": self.miscarriages,
            },
            "oxidation": {
                "ticks_fired": self.oxidation_ticks_fired,
                "ticks_threshold_open": self.oxidation_ticks_threshold_open,
                "ticks_blocked_by_empty_reserve": self.oxidation_ticks_blocked_empty_reserve,
            },
        })
    }
}

// ---------------------------------------------------------------------------------------

fn notes() -> Value {
    json!([
        "Own-cell availability is POTENTIAL access. It is the stock standing in the cell the \
         member occupies, read before any settlement touches the fields. It is not intake, and \
         no quantity in `local_cell` ever reached a reserve.",
        "An access classification counts occupancy of a cell holding stock. Neither a bare \
         encounter nor an occupancy count identifies a cause: a member standing in a stocked \
         cell may request nothing, may be outbid by co-occupants, and may assimilate a \
         fraction of what transfers.",
        "Upkeep payment is recorded against three demand terms (maintenance, movement, \
         sensing) but is deliberately NOT split across them. The core pays one lumped debit \
         clamped to the battery; only `demand_total` and `paid_total` participate in \
         reconciliation, and any per-term share of `paid` would be attribution, not \
         measurement.",
        "Survivors at the horizon are CENSORED, not successes. A member still alive when the \
         replay stops has an unfinished life; its totals are a partial observation and its \
         absence of a death record is not evidence of fitness.",
        "The build label is SUPPLIED to the encoder, not derived from the diagnostic binary. \
         `build_label_supplied` describes the bytes this program wrote; \
         `diagnostic_binary_sha256` describes the program. They are independent facts and \
         neither certifies the other.",
        "Forms 4-7 are zero-exposure. No member of those forms ever existed at this revision, \
         so every rate, ratio and share reported for them is a null ratio over an empty \
         denominator and is not a successful outcome.",
        "There is no counterfactual anywhere in this artifact. Nothing was changed, so no \
         measurement here attributes any outcome to any mechanism.",
        "`growth_gate.closest_deficit` serializes as null when the reserve side of the growth \
         predicate was never closed at an observation: the ledger holds an infinity there, \
         which JSON cannot carry.",
        "Schema is recorded as read and as written rather than assumed. Every comparison in \
         this artifact is schema 9 against schema 9."
    ])
}

/// Reserve the final path before any expensive replay. An interrupted run can leave an
/// empty/partial artifact (which readers must reject), but can never replace prior evidence.
fn reserve_output(path: &std::path::Path) -> Result<std::fs::File> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating output parent {}", parent.display()))?;
    }
    std::fs::OpenOptions::new().write(true).create_new(true).open(path)
        .with_context(|| format!("output must be a NEW file: {}", path.display()))
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut output = reserve_output(&args.out)?;
    let horizon = args.ticks;
    let cohort = &args.cohort;
    let seed = args.seed;

    let initial_manifest_path = cohort.join("initial-manifest.json");
    let manifest_path = cohort.join("manifest.json");
    let initial_snapshot_path = cohort.join(format!("initial-seeds/seed-{seed}/world-0.cubw"));
    let closing_snapshot_path = cohort.join(format!("seed-{seed}/world-144000.cubw"));
    let telemetry_path = cohort.join(format!("seed-{seed}/telemetry.jsonl"));

    let initial_manifest: Value = serde_json::from_slice(
        &std::fs::read(&initial_manifest_path)
            .with_context(|| format!("reading {}", initial_manifest_path.display()))?,
    )?;
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(&manifest_path)
            .with_context(|| format!("reading {}", manifest_path.display()))?,
    )?;
    let expected_input = initial_manifest
        .get("initials")
        .and_then(Value::as_array)
        .and_then(|l| manifest_entry(l, seed, "seed"))
        .cloned()
        .unwrap_or(Value::Null);
    let expected_closing = manifest
        .get("openings")
        .and_then(Value::as_array)
        .and_then(|l| manifest_entry(l, seed, "seed"))
        .cloned()
        .unwrap_or(Value::Null);

    let binary_sha = std::env::current_exe()
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .map(|b| sha256_hex(&b));

    // ----- Gate 1: input identity -----
    let snapshot_bytes = std::fs::read(&initial_snapshot_path)
        .with_context(|| format!("reading {}", initial_snapshot_path.display()))?;
    let (meta, retained_initial_state) = decode_snapshot(&snapshot_bytes)
        .map_err(|e| anyhow::anyhow!("{}: {e}", initial_snapshot_path.display()))?;

    let mut gate1 = Checks::new();
    let (opened_state, input_source, input_bytes): (WorldState, &str, Vec<u8>) = if args.from_seed {
        // The lever: build the world from the seed and require the constructed tick-zero
        // state to re-encode to the retained snapshot.
        let config = WorldConfig { seed, ..WorldConfig::default() };
        let world = World::new(config).map_err(|e| anyhow::anyhow!("invalid world config: {e}"))?;
        let bytes = encode_snapshot(&world.state, BUILD_LABEL);
        gate1.note(json!({
            "field": "mode",
            "computed": "constructed from seed",
            "expected": "re-encodes to the retained world-0.cubw",
            "equal": true,
        }));
        gate1.push(cmp_str(
            "constructed_tick_zero_sha256",
            &sha256_hex(&bytes),
            Some(sha256_hex(&snapshot_bytes)).as_deref(),
        ));
        gate1.push(cmp_u64(
            "constructed_tick_zero_bytes",
            bytes.len() as u64,
            Some(snapshot_bytes.len() as u64),
        ));
        (world.state, "constructed-from-seed", bytes)
    } else {
        gate1.note(json!({
            "field": "mode",
            "computed": "loaded retained tick-zero snapshot",
            "expected": "loaded retained tick-zero snapshot",
            "equal": true,
        }));
        (retained_initial_state.clone(), "retained-snapshot", snapshot_bytes.clone())
    };

    // The header and payload, split exactly as `snapshot::decode_snapshot` splits them, so
    // the payload FNV is taken over the same bytes the writer hashed.
    let (payload, _) = split_payload(&snapshot_bytes);
    let input_sha = sha256_hex(&snapshot_bytes);
    let input_payload_fnv = fnv1a(FNV_OFFSET, payload);
    let input_state_hash = state_hash(&retained_initial_state);

    gate1.push(cmp_u64("schema", u64::from(meta.schema), Some(u64::from(SCHEMA_VERSION))));
    gate1.push(cmp_u64(
        "schema_from_manifest",
        u64::from(meta.schema),
        u(&expected_input, "schema"),
    ));
    gate1.push(cmp_str(
        "build_label",
        &meta.build_id,
        expected_input.get("build").and_then(Value::as_str),
    ));
    gate1.push(cmp_u64("payload_bytes", meta.payload_len, u(&expected_input, "payload_bytes")));
    gate1.push(cmp_u64("crc32", u64::from(meta.crc32), u(&expected_input, "crc32")));
    gate1.push(cmp_str("sha256", &input_sha, expected_input.get("sha256").and_then(Value::as_str)));
    gate1.push(cmp_u64("state_hash_fnv1a64", input_state_hash, hs(&expected_input, "state_hash")));
    gate1.push(cmp_u64(
        "state_hash_recomputed_over_file_payload",
        input_payload_fnv,
        Some(input_state_hash),
    ));
    gate1.push(cmp_str(
        "decoded_state_reencodes_to_the_file",
        &sha256_hex(&encode_snapshot(&retained_initial_state, &meta.build_id)),
        Some(input_sha.clone()).as_deref(),
    ));

    // Opening census straight off the decoded state, so it is not mediated by the ledger.
    let mut opening_by_form = [0u32; 8];
    for (_, o) in opened_state.organisms.iter() {
        opening_by_form[(o.phenotype.form as usize).min(7)] += 1;
    }
    let opening_population = opened_state.organisms.len();

    // ----- The two replays -----
    eprintln!(
        "fauna-development-flow: seed {seed}, horizon {horizon} ticks, source {input_source}"
    );
    let instrumented = replay(
        World::from_state(opened_state.clone()).map_err(|e| anyhow::anyhow!("{e}"))?,
        horizon,
        true,
    );
    eprintln!("  instrumented replay done at tick {horizon}");
    let plain = replay(
        World::from_state(opened_state).map_err(|e| anyhow::anyhow!("{e}"))?,
        horizon,
        false,
    );
    eprintln!("  uninstrumented replay done at tick {horizon}");

    let ledger = instrumented.ledger.as_deref().expect("the instrumented replay attached a ledger");

    // ----- Gate 2: closing ecology identity (same schema, 9 -> 9) -----
    let closing_sha = sha256_hex(&instrumented.closing_bytes);
    let (closing_payload, closing_header_crc) = split_payload(&instrumented.closing_bytes);
    let closing_crc = crc32_of(closing_payload);
    assert_eq!(closing_crc, closing_header_crc, "the encoder's CRC and this one must agree");
    let closing_sample = instrumented.telemetry.last().cloned().unwrap_or_default();

    let mut gate2 = Checks::new();
    let gate2_asserted = horizon == 144000;
    gate2.note(json!({
        "field": "comparison",
        "computed": format!("schema {SCHEMA_VERSION} written vs schema {} read from the retained closing", u(&expected_closing, "schema").unwrap_or(0)),
        "expected": "a same-schema 9 -> 9 comparison",
        "equal": true,
    }));
    if gate2_asserted {
        let retained_closing = std::fs::read(&closing_snapshot_path)
            .with_context(|| format!("reading {}", closing_snapshot_path.display()))?;
        gate2.push(cmp_str(
            "sha256",
            &closing_sha,
            expected_closing.get("sha256").and_then(Value::as_str),
        ));
        gate2.push(cmp_str(
            "sha256_of_the_retained_file_on_disk",
            &closing_sha,
            Some(sha256_hex(&retained_closing)).as_deref(),
        ));
        gate2.push(cmp_u64(
            "payload_bytes",
            closing_payload.len() as u64,
            u(&expected_closing, "payload_bytes"),
        ));
        gate2.push(cmp_u64("crc32", u64::from(closing_crc), u(&expected_closing, "crc32")));
        gate2.push(cmp_u64(
            "state_hash_fnv1a64",
            instrumented.closing_state_hash,
            hs(&expected_closing, "state_hash"),
        ));
        gate2.push(cmp_u64(
            "ecology_hash_fnv1a64",
            instrumented.closing_ecology_hash,
            hs(&expected_closing, "ecology_hash"),
        ));
        let want_tel = expected_closing.get("telemetry").cloned().unwrap_or(Value::Null);
        gate2.push(cmp_u64(
            "closing_telemetry_tick",
            closing_sample.tick,
            u(&want_tel, "tick"),
        ));
        gate2.push(cmp_u64(
            "closing_telemetry_population",
            u64::from(closing_sample.population),
            u(&want_tel, "population"),
        ));
        let want_forms = want_tel
            .get("population_by_form")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_u64).collect::<Vec<_>>());
        let got_forms: Vec<u64> = closing_sample.population_by_form.iter().map(|&v| u64::from(v)).collect();
        let forms_ok = want_forms.as_deref() == Some(&got_forms[..]);
        gate2.ok &= forms_ok;
        gate2.note(json!({
            "field": "closing_telemetry_population_by_form",
            "computed": got_forms,
            "expected": want_forms,
            "equal": forms_ok,
        }));
        gate2.push(cmp_u64(
            "closing_telemetry_state_hash",
            closing_sample.state_hash,
            hs(&want_tel, "state_hash"),
        ));
        gate2.push(cmp_u64(
            "closing_telemetry_ecology_hash",
            closing_sample.ecology_hash,
            hs(&want_tel, "ecology_hash"),
        ));
    } else {
        gate2.note(json!({
            "field": "status",
            "computed": format!("not asserted: horizon {horizon} is short of the retained opening tick 144000"),
            "expected": null,
            "equal": null,
        }));
    }

    // ----- Gate 3: observer neutrality -----
    let mut gate3 = Checks::new();
    gate3.push(cmp_str(
        "closing_bytes_sha256",
        &closing_sha,
        Some(sha256_hex(&plain.closing_bytes)).as_deref(),
    ));
    gate3.push(cmp_u64(
        "closing_state_hash",
        instrumented.closing_state_hash,
        Some(plain.closing_state_hash),
    ));
    gate3.push(cmp_u64(
        "closing_ecology_hash",
        instrumented.closing_ecology_hash,
        Some(plain.closing_ecology_hash),
    ));
    gate3.push(cmp_u64("events_recorded", instrumented.events.len() as u64, Some(plain.events.len() as u64)));
    let events_equal = instrumented.events == plain.events;
    gate3.ok &= events_equal;
    gate3.note(json!({
        "field": "event_stream_identical_record_for_record",
        "computed": events_equal,
        "expected": true,
        "equal": events_equal,
    }));
    gate3.push(cmp_u64("events_digest_fnv1a64", instrumented.events_digest, Some(plain.events_digest)));
    gate3.push(cmp_u64(
        "per_tick_counters_rolling_digest_fnv1a64",
        instrumented.counters_digest,
        Some(plain.counters_digest),
    ));
    gate3.push(cmp_u64("ticks_compared", instrumented.ticks_stepped, Some(plain.ticks_stepped)));
    let counters_equal = instrumented.final_counters == plain.final_counters;
    gate3.ok &= counters_equal;
    gate3.note(json!({
        "field": "final_tick_counters_field_by_field",
        "computed": counters_json(&instrumented.final_counters),
        "expected": counters_json(&plain.final_counters),
        "equal": counters_equal,
    }));
    gate3.push(cmp_u64(
        "mass_residual_bits",
        instrumented.mass_residual.to_bits(),
        Some(plain.mass_residual.to_bits()),
    ));
    gate3.push(cmp_u64(
        "water_residual_bits",
        instrumented.water_residual.to_bits(),
        Some(plain.water_residual.to_bits()),
    ));
    gate3.note(json!({
        "field": "residual_values",
        "computed": { "mass": instrumented.mass_residual, "water": instrumented.water_residual },
        "expected": { "mass": plain.mass_residual, "water": plain.water_residual },
        "equal": instrumented.mass_residual.to_bits() == plain.mass_residual.to_bits()
            && instrumented.water_residual.to_bits() == plain.water_residual.to_bits(),
    }));
    gate3.push(cmp_u64(
        "telemetry_samples_compared",
        instrumented.telemetry.len() as u64,
        Some(plain.telemetry.len() as u64),
    ));
    let telemetry_equal = instrumented.telemetry == plain.telemetry;
    gate3.ok &= telemetry_equal;
    gate3.note(json!({
        "field": "telemetry_samples_identical",
        "computed": telemetry_equal,
        "expected": true,
        "equal": telemetry_equal,
    }));

    // ----- Gate 4: flow / stock reconciliation -----
    let totals = ledger.totals();
    let mut gate4 = Checks::new();
    let tolerance_ok = cubarium_core::devflow::RESIDUAL_TOLERANCE == STATED_TOLERANCE;
    gate4.ok &= tolerance_ok;
    gate4.note(json!({
        "field": "tolerance_absolute",
        "computed": cubarium_core::devflow::RESIDUAL_TOLERANCE,
        "expected": STATED_TOLERANCE,
        "equal": tolerance_ok,
    }));
    gate4.push(cmp_u64("violations", totals.violations, Some(0)));
    gate4.push(cmp_u64("unregistered_records", ledger.unregistered_records, Some(0)));
    gate4.note(json!({
        "field": "checks",
        "computed": totals.checks,
        "expected": null,
        "equal": null,
    }));
    let within = |v: f64| v <= STATED_TOLERANCE;
    let worst_ok = within(totals.worst_residual.structure)
        && within(totals.worst_residual.reserve)
        && within(totals.worst_residual.energy);
    gate4.ok &= worst_ok;
    gate4.note(json!({
        "field": "worst_residual_per_stock",
        "computed": {
            "structure": totals.worst_residual.structure,
            "reserve": totals.worst_residual.reserve,
            "energy": totals.worst_residual.energy,
        },
        "expected": { "each": format!("<= {STATED_TOLERANCE:e}") },
        "equal": worst_ok,
    }));
    gate4.note(json!({
        "field": "first_violation_tick",
        "computed": totals.first_violation_tick,
        "expected": null,
        "equal": totals.first_violation_tick.is_none(),
    }));
    gate4.note(json!({
        "field": "identities_checked",
        "computed": [
            "structure_end == structure_start + growth_structure_gained",
            "reserve_end == reserve_start + (frugivory + grazing + scavenging to_reserve) - oxidation_burned - growth_reserve_spent - reproduction_reserve_debit + refund_reserve",
            "energy_end == energy_start + channel_energy + oxidation_energy_gained - upkeep_paid - growth_build_cost - reproduction_energy_debit + refund_energy"
        ],
        "expected": null,
        "equal": null,
    }));
    gate4.note(json!({
        "field": "members_recorded",
        "computed": ledger.members.len(),
        "expected": null,
        "equal": null,
    }));

    // ----- Fifth check: census reconstructed from the per-individual records -----
    let retained_telemetry: Vec<Value> = std::fs::read_to_string(&telemetry_path)
        .with_context(|| format!("reading {}", telemetry_path.display()))?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;

    let mut samples_compared = 0u64;
    let mut first_disagreement: Option<Value> = None;
    // The reconstructed census at the retained telemetry cadence — 100 ticks, not per tick.
    // This is the cross-check's evidence, not a per-member series.
    let mut series: Vec<Value> = Vec::new();
    let mut skimmer_peak = 0u32;
    let mut skimmer_peak_tick = 0u64;
    let mut skimmer_first_zero: Option<u64> = None;
    for sample in &retained_telemetry {
        let Some(t) = u(sample, "tick") else { continue };
        if t > horizon {
            continue;
        }
        let (by_form, total) = census_at(ledger, t);
        let want_forms: Vec<u64> = sample
            .get("population_by_form")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default();
        let got_forms: Vec<u64> = by_form.iter().map(|&v| u64::from(v)).collect();
        let want_total = u(sample, "population");
        samples_compared += 1;
        if first_disagreement.is_none()
            && (want_forms != got_forms || want_total != Some(u64::from(total)))
        {
            first_disagreement = Some(json!({
                "tick": t,
                "reconstructed_by_form": got_forms.clone(),
                "retained_by_form": want_forms.clone(),
                "reconstructed_population": total,
                "retained_population": want_total,
            }));
        }
        let skimmers = by_form[3];
        if skimmers > skimmer_peak {
            skimmer_peak = skimmers;
            skimmer_peak_tick = t;
        }
        if skimmers == 0 && skimmer_first_zero.is_none() {
            skimmer_first_zero = Some(t);
        }
        series.push(json!([t, by_form, total]));
    }
    let census_ok = first_disagreement.is_none() && samples_compared > 0;
    let census = json!({
        "label": "census cross-check (fifth check, reported beside the four gates)",
        "method": "reconstructed from the per-individual born/died boundaries alone and compared \
                   against the retained telemetry sample at every 100-tick cadence",
        "samples_compared": samples_compared,
        "first_disagreeing_sample": first_disagreement,
        "passed": census_ok,
        "closing": {
            "tick": horizon,
            "reconstructed_by_form": census_at(ledger, horizon).0,
            "reconstructed_population": census_at(ledger, horizon).1,
            "retained_by_form": expected_closing
                .get("telemetry").and_then(|t| t.get("population_by_form")).cloned(),
            "retained_population": expected_closing
                .get("telemetry").and_then(|t| t.get("population")).cloned(),
            "asserted_against_manifest": gate2_asserted,
        },
        "skimmer_form_3": {
            "peak": skimmer_peak,
            "peak_tick": skimmer_peak_tick,
            "first_sampled_zero_tick": skimmer_first_zero,
        },
        "reconstructed_series_note": "[tick, population_by_form (8 slots), population] at the \
                                      retained 100-tick telemetry cadence; not a per-tick series",
        "reconstructed_series": series,
    });

    // ----- Artifact -----
    let mut form_aggs: Vec<FormAgg> = (0..8).map(|_| FormAgg::default()).collect();
    let ancestors = ancestry(&ledger.members);
    let mut members = Vec::with_capacity(ledger.members.len());
    for (id, m) in &ledger.members {
        let (root, depth) = ancestors.get(id).copied().unwrap_or((*id, 0));
        form_aggs[(m.form as usize).min(7)].observe(m, horizon);
        members.push(member_row(m, root, depth)?);
    }
    let (closing_by_form, closing_population) = census_at(ledger, horizon);

    let gates = json!({
        "gate_1_input_identity": { "passed": gate1.ok, "checks": gate1.items },
        "gate_2_closing_ecology_identity": {
            "passed": if gate2_asserted { Value::from(gate2.ok) } else { Value::Null },
            "asserted": gate2_asserted,
            "status": if gate2_asserted { "asserted" } else { "not asserted" },
            "schema_read": u(&expected_closing, "schema"),
            "schema_written": SCHEMA_VERSION,
            "checks": gate2.items,
        },
        "gate_3_observer_neutrality": { "passed": gate3.ok, "checks": gate3.items },
        "gate_4_flow_stock_reconciliation": { "passed": gate4.ok, "checks": gate4.items },
        "census_cross_check": census,
    });

    let artifact = json!({
        "kind": "fauna-development-flow",
        "schema": 1,
        "seed": seed,
        "base_revision": BASE_REVISION,
        "build_label_supplied": BUILD_LABEL,
        "diagnostic_binary_sha256": binary_sha,
        "horizon_ticks": horizon,
        "dt": DT,
        "input": {
            "source": input_source,
            "path": initial_snapshot_path.display().to_string(),
            "schema_read": meta.schema,
            "build_label_read": meta.build_id,
            "payload_bytes": meta.payload_len,
            "crc32": meta.crc32,
            "sha256": input_sha,
            "state_hash": u64s(input_state_hash),
            "state_hash_over_file_payload": u64s(input_payload_fnv),
            "bytes_on_disk": input_bytes.len(),
        },
        "expected_input": expected_input,
        "closing": {
            "path": closing_snapshot_path.display().to_string(),
            "schema_written": SCHEMA_VERSION,
            "build_label_written": BUILD_LABEL,
            "payload_bytes": closing_payload.len(),
            "crc32": closing_crc,
            "sha256": closing_sha,
            "state_hash": u64s(instrumented.closing_state_hash),
            "ecology_hash": u64s(instrumented.closing_ecology_hash),
            "telemetry_tick": closing_sample.tick,
            "population": closing_sample.population,
            "population_by_form": closing_sample.population_by_form,
        },
        "expected_closing": expected_closing,
        "gates": gates,
        "forms": (0..8).map(|i| form_aggs[i].json(i)).collect::<Vec<_>>(),
        "world": {
            "opening": {
                "tick": 0,
                "population": opening_population,
                "population_by_form": opening_by_form,
            },
            "closing": {
                "tick": horizon,
                "population_from_world": instrumented.population,
                "population_from_telemetry": closing_sample.population,
                "population_by_form_from_telemetry": closing_sample.population_by_form,
                "population_reconstructed_from_members": closing_population,
                "population_by_form_reconstructed_from_members": closing_by_form,
            },
            "totals": {
                "members_ever_recorded": ledger.members.len(),
                "life_events": instrumented.events.len(),
                "births": instrumented.events.iter().filter(|e| matches!(e, LifeEvent::Birth { .. })).count(),
                "deaths": instrumented.events.iter().filter(|e| matches!(e, LifeEvent::Death { .. })).count(),
                "reconciliation_checks": totals.checks,
            },
            "residuals": {
                "mass_residual": instrumented.mass_residual,
                "water_residual": instrumented.water_residual,
                "mass_residual_uninstrumented": plain.mass_residual,
                "water_residual_uninstrumented": plain.water_residual,
            },
        },
        "members": members,
        "notes": notes(),
    });

    let encoded = serde_json::to_vec(&artifact)?;
    output.write_all(&encoded)
        .with_context(|| format!("writing {}", args.out.display()))?;
    output.sync_all().context("syncing the completed artifact")?;

    // ----- The gate table, printed whatever the outcome -----
    println!();
    println!("fauna-development-flow  seed {seed}  horizon {horizon}  source {input_source}");
    println!("  artifact {} ({} bytes, {} members)", args.out.display(), encoded.len(), ledger.members.len());
    print_gate("gate 1  input identity", Some(gate1.ok), &gate1.items);
    print_gate(
        "gate 2  closing ecology identity (schema 9 -> 9)",
        if gate2_asserted { Some(gate2.ok) } else { None },
        &gate2.items,
    );
    print_gate("gate 3  observer neutrality", Some(gate3.ok), &gate3.items);
    print_gate("gate 4  flow/stock reconciliation", Some(gate4.ok), &gate4.items);
    println!(
        "  CHECK 5 census cross-check: {}  ({samples_compared} samples compared)",
        if census_ok { "PASS" } else { "FAIL" }
    );
    if let Some(d) = &first_disagreement {
        println!("      first disagreeing sample: {d}");
    }
    println!();

    let failed = !gate1.ok
        || (gate2_asserted && !gate2.ok)
        || !gate3.ok
        || !gate4.ok
        || !census_ok;
    if failed {
        eprintln!(
            "\nFAILED: a gate did not pass for seed {seed}. The artifact at {} is written and \
             UNREPAIRED — the failure is the result. Do not rerun expecting a different answer; \
             read the failing check above.\n",
            args.out.display()
        );
        std::process::exit(1);
    }
    Ok(())
}

fn print_gate(name: &str, passed: Option<bool>, items: &[Value]) {
    let status = match passed {
        Some(true) => "PASS",
        Some(false) => "FAIL",
        None => "NOT ASSERTED",
    };
    println!("  {name}: {status}");
    for item in items {
        let Some(obj) = item.as_object() else { continue };
        let field = obj.get("field").and_then(Value::as_str).unwrap_or("?");
        let equal = obj.get("equal").cloned().unwrap_or(Value::Null);
        let mark = match equal {
            Value::Bool(true) => "ok  ",
            Value::Bool(false) => "DIFF",
            _ => "--  ",
        };
        println!(
            "      {mark} {field}: {} | expected {}",
            compact(obj.get("computed")),
            compact(obj.get("expected"))
        );
    }
}

fn compact(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "-".to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

/// Split an encoded snapshot into its payload and the CRC32 its header declares, exactly as
/// `snapshot::decode_snapshot` splits it:
/// `[magic 4][schema 4][build_id_len 2][build_id][payload_len 8][crc32 4][payload]`.
fn split_payload(bytes: &[u8]) -> (&[u8], u32) {
    let id_len = u16::from_le_bytes(bytes[8..10].try_into().expect("2 bytes")) as usize;
    let at = 10 + id_len;
    let crc = u32::from_le_bytes(bytes[at + 8..at + 12].try_into().expect("4 bytes"));
    (&bytes[at + 12..], crc)
}

/// CRC32 (IEEE), the same polynomial `crc32fast` uses, over the encoded payload. Computed
/// here so the artifact reports the value independently of the encoder that wrote it.
fn crc32_of(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *entry = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_reservation_refuses_existing_files_directories_and_symlinks() {
        let dir = std::env::temp_dir().join(format!("cubarium-fauna-output-{}-{}",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("nested/artifact.json");
        let mut first = reserve_output(&path).unwrap();
        first.write_all(b"retained evidence").unwrap();
        assert!(reserve_output(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"retained evidence");
        assert!(reserve_output(&dir).is_err());
        #[cfg(unix)]
        {
            let link = dir.join("link.json");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(reserve_output(&link).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), b"retained evidence");
        }
        drop(first);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_interrupted_empty_reservation_cannot_be_reused() {
        let path = std::env::temp_dir().join(format!("cubarium-fauna-interrupted-{}-{}.json",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        drop(reserve_output(&path).unwrap());
        assert!(reserve_output(&path).is_err());
        assert!(std::fs::read(&path).unwrap().is_empty());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn sha256_matches_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // A message long enough to need a second padding block.
        assert_eq!(
            sha256_hex(&[b'a'; 1_000]),
            "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
        );
    }

    #[test]
    fn crc32_matches_the_encoders_header_field() {
        // The IEEE check value, and then the encoder itself: `encode_snapshot` writes the
        // CRC of the payload into the header, so recomputing it here must agree.
        assert_eq!(crc32_of(b"123456789"), 0xCBF4_3926);
        let world = World::new(WorldConfig { seed: 1, ..WorldConfig::default() })
            .expect("the default config is valid");
        let bytes = encode_snapshot(&world.state, BUILD_LABEL);
        let (payload, header_crc) = split_payload(&bytes);
        assert_eq!(crc32_of(payload), header_crc);
    }

    #[test]
    fn fnv1a_matches_the_snapshot_hash_over_a_payload() {
        let world = World::new(WorldConfig { seed: 1, ..WorldConfig::default() })
            .expect("the default config is valid");
        let bytes = encode_snapshot(&world.state, BUILD_LABEL);
        let (payload, _) = split_payload(&bytes);
        assert_eq!(fnv1a(FNV_OFFSET, payload), state_hash(&world.state));
    }

    #[test]
    fn a_member_is_absent_from_the_tick_its_removal_boundary_names() {
        // The census rule, pinned against the core's `event_tick == decision_tick + 1`.
        let mut ledger = DevFlowLedger::new(0);
        let a = OrganismId { slot: 0, generation: 1 };
        let stocks = cubarium_core::devflow::Stocks { structure: 1.0, reserve: 0.0, energy: 0.0 };
        ledger.open_tick(a, 10, 3, "founder", None, 0, stocks, 2.0, 4.0, 3.0);
        ledger.open_boundary();
        ledger.record_death(
            a,
            10,
            "starvation",
            10,
            stocks,
            None,
            cubarium_core::devflow::ToDetritus::default(),
            None,
        );
        let m = &ledger.members[&a];
        assert!(alive_at(m, 10), "still in the arena on its last stepped tick");
        assert!(!alive_at(m, 11), "removed by the boundary it died on");
    }

    #[test]
    fn the_stated_tolerance_is_the_ledgers_own() {
        assert_eq!(cubarium_core::devflow::RESIDUAL_TOLERANCE, STATED_TOLERANCE);
    }
}
