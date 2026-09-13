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
| Tick behaviour: phases, escape, contact, capture, digestion, offspring | landed |
| Deterministic test suite (25 behaviour tests + 4 migration tests) | landed |
| Paired experiment runs, ecological gates | **not started — root owns the harness** |

## API, frozen for the paired harness

This is the surface root can build the experiment entrypoint against. It compiles today
(`cargo check --workspace --all-targets --offline` is clean) and the behaviour slice added no
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

## What a tick actually does now

All of it is skipped whole when the member list is empty, so a world without hunters runs the
pre-hunter tick operation for operation, with no additional draws. The proof is the byte-exact
continuation below.

1. **Phases (after the ordinary decisions, before movement).** Each member runs its own local
   machine on a copy of its record: drop a target that died, had its slot reused, became a
   hunter, grew out of the window or stopped fitting the gut; drop a stalk or a windup that
   lost local sensing of its target; expire timed phases; perch when satiated or carrying.
   A hungry hunter with attacks enabled takes the **nearest eligible prey its own neighbour
   list already holds** — sorted by `(distance, id)`, never a global scan. When the
   transported jaw is in reach it winds up (0.6 s, no capture). At the end of the windup it
   strikes **only if the whole strike cost is available**, and that cost is charged there and
   then, before any outcome is known; otherwise the attempt is refused with an explicit
   `Unaffordable` record, no draw and no attack counted. A member never grazes or eats fruit;
   detritus scavenging is the profile's explicit fraction of its own rate, and only with no
   meal and no hunt in progress.
2. **Escape (still before movement).** Only a prey that is actually being pursued *and*
   actually senses its pursuer turns: its decided heading is rotated away within the profile's
   escape turn limit, and it may ask for up to `escape_speed_multiple` of its own maximum
   speed.
3. **Movement.** Unchanged, except that a strike burst and an escape dash are capped by
   `MoveBill::affordable_speed` — the movement energy the creature has *before* it moves —
   and never below the speed it would ordinarily have had.
4. **Capture settlement (after both creatures moved, before feeding and physiology).** Every
   paid attempt ending this tick is collected, ordered by its own seeded priority draw with
   the full ID as the tiebreak, and resolved once. Contact is re-evaluated from the
   transported jaw anchor. At most one hunter claims each prey; a contender whose target was
   claimed is told so and keeps its payment. A claimed prey is removed exactly once, with one
   ordinary `LifeEvent::Death` carrying `DeathCause::Predation`, and its whole body **and
   escrow** move into the gut — an internal transfer with no source ledger and no detritus
   cap. The three persisted natural-death counters never move.
5. **Handling and digestion (before physiology).** The handling cost is paid first and in
   full, or nothing is digested that tick. A portion leaves the gut with exactly the energy it
   was carrying, stores `eta_m · min(1, rho/e_r) · q` as reserve, rejects the rest as
   energy-free litter in the hunter's own cell, and sends the spare energy to the battery or
   to heat. Reserve headroom shrinks the **portion**, not the assimilation. A finished meal
   ends exactly empty; a satiated hunter keeps its meal, counted and checkpointed, with no
   hidden discard timer.
6. **Physiology.** A member gestates on the profile's clock, grows at the profile's juvenile
   ceiling, and buds only through the profile's own gate — local parent state only, never a
   world population count.
7. **Commit.** A member's death hands its carried gut to the cell like a body (detritus cap
   applied, remainder as heat), removes its record and every other member's handle on it. A
   funded child joins the lineage explicitly, with the fixed genome copied exactly even where
   ordinary prey mutate, an empty gut, no target and a fresh attack counter; the parent then
   waits its recovery interval. A birth the cap refuses returns the escrow through the
   existing path and makes the parent wait a gestation, so a full world cannot loop on it.

Two draws per **paid** attempt, both on the new `Stream::Hunt` keyed by the packed full hunter
ID: `2·attack_counter` for the contested-claim priority and `2·attack_counter + 1` for the
capture roll. Refusals, lookups, rendering and logging consume none. No existing stream value
moved.

## Evidence

```text
cargo test -p cubarium-core --offline          # 249 passed, 0 failed, 2 ignored
cargo clippy -p cubarium-core --offline --all-targets   # clean
cargo check --workspace --all-targets --offline         # clean, no host edit needed
```

Every hunter test runs in debug, so the world's own per-tick energy audit and invariant check
ran on every tick of every capture, digestion, birth and death below — the identity is checked
by the world itself, not only by the assertions.

### The continuation that gates everything else

`tests/hunter_migration.rs` — root's genuine `pre-hunter-v9-173400.cubw` migrates with an
empty, inert extension; 600 stepped ticks re-encode to `pre-hunter-v9-173400-plus600.cubw`
**byte for byte**, with the provenance's own hashes (`134f4db0135d8a0a`, `854dce1766d905d3`)
recomputed from the files. Fields, organisms, weather, config, care, the signed corrections and
the raw counters are compared individually. The genuine schema 8 and schema 7 continuations in
`care.rs` and `energy_correction.rs` still pass unchanged.

### `tests/hunter.rs`, 25 deterministic tests

Two trial parameters are replaced in these fixtures to make outcomes deterministic:
`capture_min = capture_max = 1` for a certain capture and `= 0` for a certain miss.

- **Founding.** The derived inventory is the world's own decode (`S = 2, R = 2, E = 3`, 4 m and
  7 e), booked once in the extension and never in `external_material_in`; the closed box and
  the stored energy move by exactly the import. Repeats, an invalid profile, a body too big for
  the world's `body_extent_max` and an unresolvable target are each refused **without changing
  anything**.
- **Budget-matched control.** The same derived inventory deposited as local `D`/`De`, no
  hunter, mutually exclusive with the trial arm, no repeat. With a detritus cap too small to
  hold it, the excess becomes real heat and the energy identity still closes.
- **The hunt.** Perch → stalk → windup (which captures nothing) → paid strike → settlement.
  The strike cost is charged at entry; an unaffordable strike is refused before payment and
  consumes no draw and no counter; a miss still pays and leaves the prey alive.
- **Capture.** The whole prey **and its escrow** move into the gut (material exact, energy
  within the tick's own movement cost), the body does not also become detritus, material is
  closed, the energy identity holds, exactly one `Death` event with `DeathCause::Predation` is
  emitted and the three natural counters stay at zero.
- **Eligibility.** Prey too big, too small, or whose body does not fit the remaining gut is
  never stalked and never attempted.
- **Contested claim.** Two hunters, one prey: one `Captured`, one `TargetClaimed`, two paid
  attempts, one carried body, one death. (This test found a real defect: the loser was
  originally told `TargetLost` because the winner's settlement had already cleared its handle.)
- **Escape.** A stalked prey turns away from its pursuer and exceeds its own maximum speed.
- **Digestion.** The split matches the plan's formula exactly, tick by tick, in a quiet world
  where nothing else can move the litter; material and the energy identity hold on every tick
  of the meal. A full reserve stops digestion and the gut keeps the meal; a hunter that cannot
  pay the handling cost digests nothing.
- **Scavenging.** A facultative hunter gains reserve from litter at its allocated fraction; a
  specialist gains nothing. Neither ever grazes or eats fruit.
- **Attacks disabled.** The same living hunter is still maintained and still spends, attempts
  nothing and logs nothing.
- **Death.** A hunter that starves hands its carried meal to the cell with its body, capped
  like a body with the remainder as heat, leaves the member list, and closes both identities.
- **Geometry.** The jaw reaches **across a seam** onto a face the body is not on, and
  **reflects off the open rim**, and captures in both cases; `HunterView.mouth` is the
  transported anchor.
- **One funded offspring.** The escrow is funded at the world's own child fractions out of the
  parent; the child is a member with the fixed genome copied exactly **with mutation on**, the
  profile's extent, a fresh attack counter, and the parent starts its recovery interval;
  material and energy identities hold across the birth. A parent that dies miscarries once and
  leaves the lineage; a birth refused by the cap returns the escrow and waits a gestation.
- **Restart.** Checkpoint and reload mid-windup, mid-paid-strike, with a part-filled gut and
  mid-gestation: identical full-state hashes at the boundary and at every tick for 120 ticks
  after, and identical extensions at the end.
- **Stale handles.** After a capture the freed slot is reused by a new animal; a replanted
  stale handle is cleared rather than resolved, and the new occupant is not harvested for it.
- **Decode hardening.** Thirteen crafted extensions are refused by name: a member that is not
  a live organism, duplicates, a gut past capacity, energy in an empty gut, handling nothing,
  stalking nobody, a timed phase with no duration, an untimed phase that stores one, a hunter
  aiming at itself, members without a profile, a control world holding a hunter, a negative
  import, and an unknown profile version.

## Open, and not claimed

- **No ecological claim at all.** These are unit and integration tests of mechanism, not
  evidence about balance. Long-run prey margins are *not* validated: the twelve-hour
  comparisons retain total population but lose skimmers and founder diversity, so a green
  suite is not a reason to put this lineage anywhere near the live world.
- **The paired experiment has not been run.** Root owns the harness and the runs: twelve
  paired seeds from documented mature snapshots, stratified by initial prey population, with
  the four arms the plan names (untouched baseline, budget-matched deposit, attack-disabled
  living hunter, specialist and facultative hunters), two hours per seed before anything
  longer.
- **The jaw geometry is unconfirmed.** Six pixels forward with 1.5 px reach are placeholders;
  the art worker must confirm that `HunterView.mouth` tracks the visible jaw before any
  integration. The 9 px body extent is the world's own `body_extent_max`, not a measurement of
  the assembled silhouette.
- **Untuned trial numbers with a known demand.** At the founder defaults, maintenance alone is
  about 18 e/hour before sensing and movement; how much prey turnover that actually requires
  is a measurement nobody has made yet.

## Remaining gates before this lineage goes anywhere

1. Root's paired runs pass their accounting checks and do not universally collapse the prey.
2. Fable confirms the jaw anchor and maps `HunterRole::Lanternjaw` to the body; only then does
   the render integration land.
3. Wrysk's explicit authorization for any live introduction or any default change. Nothing in
   this package alters a default: an untouched world has no profile, no members and no
   imports, and steps exactly as the pre-hunter build did.

## Deviations from the plan, and why

- **Schema 10, not the plan's schema 9.** The accounting correction took 9 (`b47eacc`), as the
  work order says; the hunter extension is appended after `energy_correction`, and the frozen
  `WorldStateV9` mirror preserves the whole schema 9 prefix.
- **Two draws per paid attempt, not one.** The plan asks for "one draw per paid attempt" and
  also for a "dedicated seeded draw" for contested-claim priority. Deriving both from one draw
  would correlate a hunter's claim priority with its own capture roll, so the priority and the
  capture roll are separate counters in the same stream. Nothing else consumes a draw.
- **`RenderView` and `OrganismView` untouched.** Per the work order, the hunter view is
  published separately so Fable's concurrent host work is not forced to change.
- **A refused birth waits one gestation**, not the full reproduction interval: long enough that
  a world at its cap cannot retry every tick, short enough that a refusal is not a de facto
  half-hour ban. The plan asks only for "a retry/recovery interval".
