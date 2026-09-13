---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Opt-in paid Lanternjaw hunter: core implementation progress

Implementation record for `fixed-hunter-core-handoff-2026-09-13.md`, following the recipe in
`astra-fixed-hunter-implementation-plan-2026-09-13.md`. Evidence, not decisions: **no number
here is validated balance**, nothing is on by default, and nothing has been introduced into
the live world.

Scope edited: `crates/cubarium-core/` and this file. No host, renderer, art, experiment
harness, live state or care file was touched.

## Status

| Slice | State |
| --- | --- |
| Extension types, profile, validation, pure rules | landed |
| Schema 10 with frozen schema 9 mirror, 9/8/7 migrations | landed |
| Initializers (`start_hunter_trial`, budget-matched control) | landed |
| Observer API (`hunter_view`, `drain_hunter_events`, telemetry) | landed |
| Gut in stored totals, invariants and both energy audits | landed |
| Tick behaviour: phases, escape, contact, capture, digestion, offspring | in progress |

## API, frozen for the paired harness

This is the surface root can build the experiment entrypoint against. It compiles today
(`cargo check --workspace --all-targets --offline` is clean) and the behaviour slice adds no
new signature to it.

```rust
// crates/cubarium-core/src/hunter.rs  (re-exported from the crate root)
pub const PROFILE_VERSION: u32 = 1;

pub enum HunterRole { Lanternjaw }           // semantic role; genome.form only picks a rig
pub struct HunterTarget { face: u8, u: f64, v: f64 }   // resolve() -> Option<SurfacePoint>

pub struct FixedHunterProfile {
    version: u32, role: HunterRole, genome: Genome, attacks_enabled: bool,
    body_extent_px, jaw_offset_px, jaw_reach_px,
    founder_reserve_fraction, founder_energy_fraction,
    perch_reserve_fraction, seek_reserve_fraction,
    prey_structure_min, prey_structure_fraction_max,
    stalk_timeout_seconds, windup_seconds, strike_seconds, strike_speed_px_s,
    strike_energy_cost, recovery_seconds,
    capture_base, capture_min, capture_max,
    escape_speed_multiple, escape_turn_rate_deg,
    gut_capacity_material, handling_cost_per_second, digest_rate, meal_recovery_seconds,
    scavenge_fraction,
    reproduce_min_age_seconds, reproduce_reserve_fraction, reproduce_energy_fraction,
    reproduce_interval_seconds, gestation_seconds, juvenile_growth_rate,   // all f64
}
impl FixedHunterProfile {
    fn lanternjaw_trial(config: &WorldConfig) -> Self;   // the trial placeholders below
    fn facultative(self) -> Self;                        // scavenge_fraction = 0.25
    fn without_attacks(self) -> Self;                    // attacks_enabled = false
    fn validate(&self) -> Result<(), String>;
}

pub enum HunterPhase { Perched, Stalking, Windup, Strike, Recovering, Handling }
pub struct HunterMember {
    id: OrganismId, phase: HunterPhase, phase_started_tick: u64, phase_ends_tick: u64,
    target: Option<OrganismId>, attack_counter: u64, next_reproduction_tick: u64,
    gut_material: f64, gut_energy: f64,
}
pub struct HunterState {            // WorldState.hunters, appended at schema 10
    profile: Option<FixedHunterProfile>, members: Vec<HunterMember>,   // sorted by full ID
    founder_material_in, founder_energy_in, control_material_in, control_energy_in: f64,
    control_deposited: bool, founders_placed: u32,
    attacks_total, captures_total, predation_deaths_total,
    hunter_deaths_total, hunter_births_total: u64,
}

// world
impl World {
    pub fn hunters(&self) -> &HunterState;
    pub fn start_hunter_trial(&mut self, profile: FixedHunterProfile, target: HunterTarget)
        -> Result<HunterFounderReceipt, String>;
    pub fn deposit_hunter_budget_control(&mut self, profile: FixedHunterProfile, target: HunterTarget)
        -> Result<HunterControlReceipt, String>;
    pub fn hunter_view(&self) -> Vec<HunterView>;
    pub fn drain_hunter_events(&mut self) -> Vec<HunterEvent>;
}

pub struct HunterFounderReceipt {   // what was actually placed and booked
    id: OrganismId, tick: u64, pos: SurfacePoint,
    structure: f64, reserve: f64, energy: f64, material_in: f64, energy_in: f64,
    extent: f64, sense_radius: f64, jaw_offset_px: f64, jaw_reach_px: f64,
}
pub struct HunterControlReceipt {   // the same inventory, deposited as local D/De
    tick: u64, cell: u16, material_in: f64,
    energy_in: f64, energy_stored: f64, energy_heat: f64,
}
pub struct HunterView {             // one per live member, keyed by full ID
    id, role, phase, phase_progress: Option<f32>, pos, heading,
    mouth: SurfacePoint, mouth_reach_px: f64, target: Option<OrganismId>,
    structure, extent, juvenile, gut_material, gut_energy, gut_fraction: f32,
    gestation: Option<f32>,
}
pub enum HunterEvent {              // transient, drained like LifeEvent
    Attempt { tick, hunter, target: Option<OrganismId>, outcome: AttemptOutcome, energy_paid: f64 },
    Capture { tick, hunter, prey, material: f64, energy: f64 },
    Offspring { tick, parent, child },
    Death { tick, id, cause: DeathCause, gut_material, gut_energy, gut_energy_stored },
}
pub enum AttemptOutcome {
    Captured, Missed, OutOfReach, TargetLost, TargetClaimed, Ineligible, Unaffordable,
}
```

`Telemetry` gained (all `#[serde(default)]`, all zero without hunters): `hunters`,
`hunter_juveniles`, `hunter_attacks`, `hunter_captures`, `deaths_predation` (per sample, like
`births`), `hunter_gut_material`, `hunter_gut_energy`, `hunter_attacks_total`,
`hunter_captures_total`, `predation_deaths_total`, `hunter_births_total`,
`hunter_deaths_total`, `hunter_material_in`, `hunter_energy_in`. `RenderView` and
`OrganismView` are **unchanged**, as the work order requires.

`DeathCause` gained an appended `Predation`; the three natural causes keep their order and
their index in the persisted `deaths_total` triple. `LifeEvent` gained no variant: a consumed
prey emits exactly one ordinary `LifeEvent::Death` with the new cause.

### Both arms of an experiment

```rust
let profile = FixedHunterProfile::lanternjaw_trial(world.config());   // or .facultative() / .without_attacks()
let target = HunterTarget { face: 4, u: 32.0, v: 32.0 };
let receipt = world.start_hunter_trial(profile.clone(), target)?;     // hunter arm
let control = other.deposit_hunter_budget_control(profile, target)?;  // budget-matched arm
```

Both validate everything before touching the world, refuse a repeat, refuse the other arm's
world, and return the amounts actually booked. Neither resets the world's opening history: the
import is booked in the extension, so `mass_residual()` is unmoved by the initialization
itself (asserted in debug).

## Accounting

The extension is inside every audit rather than beside them:

```text
mass_residual  += Σ gut_material − (founder_material_in + control_material_in)
stored_energy  += Σ gut_energy
energy sources += (founder_energy_in + control_energy_in)
```

`World::from_state` re-derives its baseline with the same terms, so a resumed world still opens
at a zero residual. A capture, a digestion and a hunter's death are **internal transfers**: they
move no term of the energy identity except heat, which goes through the same compensated
accumulator (`crate::accounting`) as every other payment.

## Schema 10

`SCHEMA_VERSION` is now **10**; schema 10 is schema 9 plus exactly `WorldState.hunters`, 41
bytes when empty. `snapshot/v9.rs` freezes `WorldStateV9` (the field list at root's `1f0fc3a`)
with `project()` and a migration that opens the extension empty. `decode_snapshot` accepts
10, 9, 8 and 7 and reports which it read; 6 and older are still refused. `ecology_hash` is
still the schema 7 projection, so existing care/no-care comparisons are unaffected.

## Trial parameters (placeholders, not balance)

Founder genome `size 2, reserve 2, mouth 1, speed 1, sense 8, metabolism 0.5, depth 0.85,
swim 0.1, diet 0`, form 4, role `Lanternjaw`. At the default config that decodes to
`S_adult = 2`, `R_max = 4`, `E_max = 4`, `v_max ≈ 0.2523 px/s`, maintenance `0.0025` per
structural unit per second, and the derived founder inventory is `S = 2, R = 2, E = 3` —
**4 material and 7 energy**, derived from the config, never hardcoded.

Behaviour: perch above 65 % reserve, hunt below 35 %; prey `0.15 ≤ S ≤ 0.75 · S_hunter` whose
whole inventory fits the 4 m gut; 8 s stalk timeout; 0.6 s windup (no capture); 1 s strike at
up to 1 px/s costing 0.08 e, charged in full at entry; capture
`clamp(0.65 · S_h/(S_h + S_p), 0.1, 0.75)`; 5 s recovery after a failed attempt; handling
0.002 e/s; digestion 0.1 m/s; 20 s pause after a meal; escape up to 2× the prey's own maximum
speed within 240 °/s, both limited by the movement energy it has. Reproduction: full adult
structure, 1200 s age, 80 % reserve, 75 % energy, no gut, no hunt, 1800 s since the last birth,
120 s gestation, juvenile growth 0.002 m/s. The six-pixel jaw offset and 1.5 px reach are
**placeholders the art worker must confirm against the visible jaw**; `HunterView.mouth` is the
transported anchor contact is actually tested at.

## Evidence so far

```text
cargo test -p cubarium-core --offline          # 224 passed, 0 failed, 2 ignored
cargo clippy -p cubarium-core --offline --all-targets   # clean
cargo check --workspace --all-targets --offline         # clean, no host edit needed
```

- `tests/hunter_migration.rs`: root's genuine `pre-hunter-v9-173400.cubw` migrates with an
  empty extension; 600 stepped ticks re-encode to `pre-hunter-v9-173400-plus600.cubw`
  **byte for byte**, with the provenance's own hashes (`134f4db0135d8a0a`,
  `854dce1766d905d3`) recomputed from the files; care, the signed corrections and the raw
  counters are compared individually. The genuine schema 8 and schema 7 continuations in
  `care.rs` and `energy_correction.rs` still pass unchanged.
- `src/hunter.rs` unit tests: profile validation rejects ten separate defects; capture
  probability is bounded and falls with prey size; digestion conserves material and energy
  exactly, stores proportionally less from energy-poor prey, reduces the portion rather than
  the assimilation under reserve headroom, and sends spare energy to heat with a full battery;
  the boost speed cap never slows ordinary movement; member bookkeeping stays sorted, is
  generation-checked and forgets removed IDs.

## Open, and not claimed

- The tick behaviour is not finished yet; nothing hunts in this slice.
- No ecological claim at all. Long-run prey margins are **not** validated: the twelve-hour
  comparisons retain total population but lose skimmers and founder diversity, so a passing
  unit test is not evidence that this lineage belongs in the live world.
- The jaw geometry is unconfirmed by the art worker.
