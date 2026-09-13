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
| Schema 11 with frozen 10/9/8/7 mirrors and the schema 10 refusal | landed |
| Initializers (`start_hunter_trial`, budget-matched control) | landed |
| Observer API (`hunter_view`, `drain_hunter_events`, telemetry) | landed |
| Gut in stored totals, invariants and both energy audits | landed |
| Tick behaviour: phases, escape, contact, capture, digestion, offspring | landed |
| Root-chart contact geometry, measured claws, shared body scale | landed |
| Settlement metadata on events, timing facts in the view | landed |
| Astra review corrections: NaN bounds, settlement boundaries, authored claw | landed |
| Exact reproduction transaction evidence (`5ee8fee`) | landed |
| Read-only reproduction observer for the harness (`d7d190a`, `f4c410a`) | landed, awaiting root's wiring |
| Deterministic test suite (27 behaviour + 8 geometry + 6 migration + 7 reproduction) | landed |
| Paired experiment runs, ecological gates | **not started — root owns the harness** |

**Read this section if you integrated against an earlier delivery.** The geometry correction
of `lanternjaw-core-art-integration-gaps-2026-09-13.md` changed the profile's shape, so the
snapshot is **schema 11**; Astra's `astra-hunter-geometry-review-2026-09-13.md` then found three
defects, all now fixed, one of which moved a frozen constant. The profile is
**`PROFILE_VERSION 3`**. Nothing in root's harness broke —
`cargo check --workspace --all-targets --offline` is clean, because the harness matches event
variants with `..` — but these changed meaning:

- `profile.jaw_offset_px` / `jaw_reach_px` are gone. The capture effector is
  `capture_offset_body: Vec2` with `capture_reach_px`, and the ingestion mouth is its own
  `ingestion_offset_body`.
- `HunterView.mouth` is gone. `capture_center` and `ingestion_center` are `Option`, and they
  are `None` exactly when the point cannot honestly be drawn there.
- **Post-movement phases now open at the boundary their tick completes.** A capture's
  `Handling` and a paid miss's `Recovering` start at `now + 1` — the same tick their event is
  stamped with — instead of a tick earlier. Anything that interpolated a recoil from the
  published entry was starting it before the contact.
- **The capture effector's side coordinate is `1.1`,** Fable's authored value, not the
  `1.162368` of the contract's earlier decorated-study measurement. That is why the profile is
  version 3: a frozen experimental constant moved, so a saved version 2 trial is refused by
  name rather than silently re-measured.
- Any archived schema 10 artifact with a **non-empty** hunter extension — a trial, a
  budget-matched control, or an extinct lineage's counters — no longer loads: it is refused by
  name. Empty extensions still migrate exactly.
- **`HunterEvent` gained one variant**, `Reproduction { tick, hunter, record }` (`5ee8fee`).
  That is a deliberate, temporary host break at exactly one place:
  `crates/cubarium/examples/hunter_compare.rs:377`, whose `match event { … }` exists only to
  read a tick. `HunterEvent::tick()` now answers that for every variant, so the whole match can
  become `event.tick()`; `HunterEvent::hunter()` does the same for the member. Root owns that
  file and this worker did not touch it. Nothing else in the workspace is affected, and no
  existing variant or field changed.

## Final API, for root and Fable

Everything below compiles today (`cargo check --workspace --all-targets --offline` clean) and
is what the behaviour actually uses: the world tests contact with the same numbers it
publishes.

```rust
// crates/cubarium-core/src/hunter.rs  (re-exported from the crate root)
pub const PROFILE_VERSION: u32 = 3;      // versions 1 and 2 are refused, never reinterpreted
pub const GRASP_EPS: f64 = 1e-6;         // how exactly a grasp centre must round-trip

pub enum HunterRole { Lanternjaw }       // semantic role; genome.form only picks a rig
pub struct HunterTarget { face: u8, u: f64, v: f64 }

pub struct FixedHunterProfile {
    version, role, genome, attacks_enabled,
    // three extents, kept distinct on purpose
    body_extent_px,            // physical crowding extent (pair pass, repulsion) — 9.0
    visual_query_extent_px,    // artwork query support around the root — 16.0
    capture_offset_body: Vec2, // AUTHORED near claw, body-local (13.2794117647, 1.1)
    capture_reach_px,          // trial grasp tolerance — 1.5
    ingestion_offset_body: Vec2,   // the mouth, separately — (9.6, 0.0) at full extension
    body_scale_exponent, body_scale_min,   // scale = max(min, (S/S_adult)^exponent) — 0.5, 0.2
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
    reproduce_interval_seconds, gestation_seconds, juvenile_growth_rate,
}
impl FixedHunterProfile {
    fn lanternjaw_trial(config: &WorldConfig) -> Self;   // the trial placeholders
    fn facultative(self) -> Self;      // scavenge_fraction = 0.25
    fn without_attacks(self) -> Self;  // attacks_enabled = false
    fn validate(&self) -> Result<(), String>;
}

// ---- the one shared geometry surface. Core owns it; the world and the observer use it.
pub fn body_basis(heading: Vec2) -> Option<(Vec2, Vec2)>;   // (+x forward, +y clockwise side)
pub fn body_scale(profile, structure, structure_adult) -> f64;

pub struct ContactGeometry {        // already scaled
    scale, capture_offset_body: Vec2, capture_reach_px,
    ingestion_offset_body: Vec2, visual_query_extent_px,
}
impl ContactGeometry { fn of(profile, organism) -> Self; }

pub struct ContactMeasure { body: Vec2, root_distance, effector_distance, tolerance }
impl ContactMeasure { fn in_contact(&self) -> bool; }

/// Unfold the prey FROM THE HUNTER ROOT through the rig's own shortest image, rotate into the
/// body basis, compare with the scaled effector. This is the contact authority.
pub fn measure_contact(images, root, heading, &geometry, prey_pos, prey_extent)
    -> Option<ContactMeasure>;

/// The surface point of a scaled body-local offset, or None when it cannot honestly be drawn
/// there: the sweep reflected off the open rim, hit the fallback, resolved a vertex tie, or
/// does not round-trip to the body coordinate it was built from.
pub fn body_point(images, root, heading, offset_body: Vec2) -> Option<SurfacePoint>;

pub struct ContactEvidence {        // the settlement, captured before removal
    prey: OrganismId, prey_pos: SurfacePoint, prey_extent: f64,
    hunter_pos: SurfacePoint, hunter_heading: Vec2,
    geometry: ContactGeometry,
    capture_center: Option<SurfacePoint>, ingestion_center: Option<SurfacePoint>,
    measure: Option<ContactMeasure>,
}
impl ContactEvidence {
    fn gather(images, profile, hunter, prey_id, prey) -> Self;
    fn grants_capture(&self) -> bool;   // in contact AND a drawable grasp centre
}

pub enum HunterPhase { Perched, Stalking, Windup, Strike, Recovering, Handling }
pub struct HunterMember {
    id, phase, phase_started_tick, phase_ends_tick, target, attack_counter,
    next_reproduction_tick, gut_material, gut_energy,
    entered_from: HunterPhase,   // persisted transition origin
    episode: u64,                // the paid attempt this phase belongs to, 0 if none
}
pub struct HunterState { profile: Option<..>, members: Vec<HunterMember>, /* ledgers, counters */ }

// world
impl World {
    fn hunters(&self) -> &HunterState;
    fn start_hunter_trial(&mut self, profile, target) -> Result<HunterFounderReceipt, String>;
    fn deposit_hunter_budget_control(&mut self, profile, target)
        -> Result<HunterControlReceipt, String>;
    fn hunter_view(&self) -> Vec<HunterView>;
    fn drain_hunter_events(&mut self) -> Vec<HunterEvent>;
}

pub struct HunterFounderReceipt {
    id, tick, pos, structure, reserve, energy, material_in, energy_in,
    extent, sense_radius, geometry: ContactGeometry,
}
pub struct HunterControlReceipt { tick, cell, material_in, energy_in, energy_stored, energy_heat }

pub struct HunterView {
    id, role, phase,
    phase_progress: Option<f32>,        // convenience only
    phase_started_tick, phase_ends_tick, entered_from, episode, attack_counter,
    pos, heading,
    geometry: ContactGeometry, body_scale: f64,
    capture_center: Option<SurfacePoint>, ingestion_center: Option<SurfacePoint>,
    target: Option<OrganismId>,
    structure, structure_adult, extent, juvenile,
    gut_material, gut_energy, gut_fraction: f32, gut_capacity,
    gestation: Option<f32>,
}

pub enum HunterEvent {
    Attempt { tick, hunter, target: Option<OrganismId>, outcome: AttemptOutcome,
              energy_paid: f64,
              attack_counter: Option<u64>,          // None on an unpaid refusal
              evidence: Option<ContactEvidence> },  // None when the target did not resolve
    Capture { tick, hunter, prey, material, energy,
              attack_counter: u64, evidence: ContactEvidence },
    Offspring { tick, parent, child },              // unchanged
    Reproduction { tick, hunter, record: Reproduction },   // new in 5ee8fee
    Death { tick, id, cause, gut_material, gut_energy, gut_energy_stored },
}
impl HunterEvent {
    pub fn tick(&self) -> u64;            // every variant, so no host-side exhaustive match
    pub fn hunter(&self) -> OrganismId;   // attacker / captor / parent / deceased
}

// ---- the reproduction transaction, recorded at the mutation that moved it
pub struct EscrowKey { parent: OrganismId, started_tick: u64 }   // the world's own Escrow.started_tick
pub enum FundingBlocked { Cap, Stocks }

pub enum Reproduction {
    Funded {
        key: EscrowKey,
        parent_reserve_before, parent_reserve_after,
        parent_energy_before, parent_energy_after,
        escrow_structure, escrow_reserve, escrow_energy,
        build_heat,                       // build_cost · escrow_structure
    },
    Born {
        key: EscrowKey, child: OrganismId,
        child_structure, child_reserve, child_energy,
        birth_heat,                       // e_r · escrow_structure
    },
    Refunded {                            // a due birth the cap refused: nothing burned
        key: EscrowKey,
        refunded_structure, refunded_reserve, refunded_energy,
        parent_reserve_before, parent_reserve_after,
        parent_energy_before, parent_energy_after,
    },
    Miscarried {                          // the parent died holding it; body and gut are separate
        key: EscrowKey, cause: DeathCause,
        material, energy, energy_stored, energy_heat,
    },
    NotFunded { parent: OrganismId, reason: FundingBlocked },   // no escrow ever existed
}
impl Reproduction {
    pub fn key(&self) -> Option<EscrowKey>;   // None only for NotFunded
    pub fn parent(&self) -> OrganismId;
}
pub enum AttemptOutcome {
    Captured, Missed, OutOfReach, TargetLost, TargetClaimed, Ineligible,
    GraspUnmapped,   // in reach, but of a grasp the artwork would clip — still paid
    Unaffordable,    // refused before payment: no cost, no draw, no counter
}
```

`Telemetry` is unchanged from the first delivery. `RenderView`, `OrganismView` and `LifeEvent`
still have exactly their old fields.

### The reproduction transactions, and what closes what

A gestation is named by `EscrowKey { parent, started_tick }` — the parent's full ID and the
`started_tick` the world already persists in its `Escrow`. A parent holds at most one escrow, so
the key names exactly one transaction from its funding to whichever way it ends, and a reader
can close every transaction it opens:

```text
Funded  ──▶ Born       the escrow became the child (also: Offspring, LifeEvent::Birth)
        ──▶ Refunded   a due birth the cap refused; everything went back, nothing burned
        ──▶ Miscarried the parent died holding it; the escrow alone went to the litter
NotFunded              no escrow ever existed — a non-transaction, no key, nothing to close
```

Identities the records satisfy exactly, by construction and by test:

```text
Funded:     reserve_before − reserve_after = escrow_structure + escrow_reserve
            (e_r·reserve + energy)_before − (e_r·reserve + energy)_after
                = e_r·(escrow_structure + escrow_reserve) + escrow_energy + build_heat
Born:       child S/R/E = the escrow's own S/R/E;  birth_heat = e_r · escrow_structure
Refunded:   reserve_after − reserve_before = refunded_structure + refunded_reserve
            energy_after − energy_before  = refunded_energy
Miscarried: material = escrow S + R;  energy = e_r·material + escrow E
            energy_stored + energy_heat = energy;  energy_stored ≤ energy_cap · material
```

Two things worth saying plainly. **A funding and its loss can happen in the same tick** — the
bud gate runs before the death check in the same physiology pass — and both records are emitted,
so a reader never sees a gestation end that it never saw begin. And **a miscarriage is not the
corpse**: `Miscarried` carries the escrow's own terms only; the body's material and energy and
any carried gut are separate terms of the same death, the gut reported in `HunterEvent::Death`.

Every number is read on either side of the assignment that moved it. A post-step difference
cannot substitute: oxidation, growth, movement and a death can all touch the same parent in the
same tick.

### The observer that reads them: `ReproductionAudit`

`crates/cubarium/examples/hunter_compare/reproduction.rs` (`d7d190a`, `f4c410a`) is a read-only
audit of those transactions, written to root's observer conventions and **not yet wired**: root
owns `hunter_compare.rs` and the integration.

```rust
pub struct ReproductionAudit;
impl ReproductionAudit {
    pub fn new(state: &WorldState) -> anyhow::Result<Self>;   // refuses a mid-gestation opening
    pub fn observe(&mut self, events: &[HunterEvent], life: &[LifeEvent], state: &WorldState)
        -> anyhow::Result<()>;                                // one completed tick, consecutive
    pub fn summary(&self) -> serde_json::Value;
    pub fn open_gestations(&self) -> usize;                   // censored at the horizon
    pub fn last_complete_tick(&self) -> u64;
}
```

Wiring is three lines: `#[path = "hunter_compare/reproduction.rs"] mod reproduction;` beside the
existing module declarations, one `ReproductionAudit::new(&world.state)?` per arm at the
post-initialization opening (before any member can have started a gestation), and
`audit.observe(&hunter_events, &life_events, &world.state)?` with the batches already drained
each tick. `summary()` drops into the arm summary as a JSON object.

**What it validates, per tick, committing only after the whole tick passes.** Consecutive tick
coverage; every record stamped with that tick; the wrapper's `hunter` equal to the record's own
`parent()`; a funding's `started_tick` being the tick it opened in; one outstanding gestation per
parent; no second funding, no repeated or unknown closure, no stale generation on either side;
`Born` reconciled one-for-one against both the `Offspring` record and the ordinary
`LifeEvent::Birth` (and every hunter birth having one); a `Miscarried` matched to that parent's
death, with the same cause, in the same tick; and, at the end of the tick, the open gestations
reconciled against the escrows the world is **actually holding** — key, start tick and inventory.
A funding and its loss inside one tick are processed in order and both counted.

**What it recomputes independently**, from the world's config and the parent's own decoded size
(cached per member, so a parent that funds and dies in one tick can still be checked): the
funding debit against the escrow it bought, in material and in energy; the build heat against
`build_cost · S`; the child inventory against the configured fractions; the birth heat against
`e_r · S`; a refund against what it returned; and a miscarriage's split against
`min(energy, energy_cap · material)`. Aggregates accumulate through core's own
`accounting::accumulate`, so a seventy-two-hour arm's totals are compensated.

**What it does not claim.** The quantities are core's mutation evidence: no observer outside the
tick can measure them, and the summary says so in a `provenance` field. It never treats post-step
parent stocks as an isolated funding difference. It checks that a reported heat is arithmetically
right, **not** that exactly that much heat entered the world's ledger — a tick's heat is the sum
of every source in it, and this audit does not attribute it. And it refuses a mid-gestation
attachment rather than inventing the debits that paid for an escrow already in flight.

Seventeen inline tests drive genuine streams from real breeder worlds (funded and born, a cap
refund, a funding and an age death in one tick, an ordinary miscarriage, blocked funding counted
separately by cap and by stocks), then a refusal battery — dropped, duplicated and reordered
closures, a duplicated funding, stale generations on either side, an invented heat, a child that
is not the escrow, a NaN, a wrapper disagreeing with its record, a record from another tick, a
skipped tick, a mid-gestation opening — each rejected **atomically**, with the genuine batch
still accepted afterwards. Memory stays bounded over repeated outcomes, and a run with the audit
ends on the same world hash as one without it.

Until root wires it, the module is not compiled by the workspace. It was built and tested
through a throwaway fixture that includes the file by path
(`/tmp/cubarium-reproduction-fixture-sBPNoX`, disposable): **17 passed, 0 failed**, clippy clean.
Once wired it runs under
`cargo test -p cubarium --example hunter_compare --test hunter_observers --offline`.

### The geometry contract, in one paragraph

Body coordinates are `stamp_rig`'s: `+x` along the heading, `+y` its clockwise side
(`side = (-h.y, h.x)`), and a part at body offset `o` is drawn at chart offset `scale · o` from
the root. Core measures contact by unfolding the **prey** from the **hunter root** through the
same shortest image the rig is drawn through, rotating into that basis, and comparing with
`geometry.capture_offset_body` inside `geometry.capture_reach_px + prey extent`. A physical
centre (`capture_center`, `ingestion_center`) is published only when its sweep neither reflects
nor falls back nor resolves a vertex tie **and** round-trips through the root's own unfolding to
the body coordinate it was built from, within `GRASP_EPS`. Otherwise it is `None` — the world
never publishes, and never captures from, a point the renderer would clip. Pass `body_scale`
straight to `stamp_rig_scaled`; do not derive a second scale from `juvenile` or from `extent`.

### What a phase boundary means

`phase_started_tick` and `phase_ends_tick` are **completed-tick boundaries**, and which boundary
a transition gets depends on where in the tick it was decided:

- **Pre-movement decisions** — perch, stalk, the windup, and the strike entry that charges for
  itself — are taken in the decision pass with `now` completed ticks, and are stamped `now`.
  Their effect is first *observable* one step later, the same one-step lag every decision has.
- **Post-movement settlements** — a capture's `Handling`, a paid miss's `Recovering`, and the
  pause that opens when a gut empties during digestion — belong to the tick they complete and
  are stamped `now + 1`, exactly the tick their `HunterEvent` carries. A newborn member joins at
  `now + 1` for the same reason.

A timed phase occupies the closed interval `[phase_started_tick, phase_ends_tick]`: the decision
pass leaves it when the world has *reached* `ends`, so the member is still in it when observed
at `ends` and has moved on one step later, and the next phase's `phase_started_tick` is exactly
this one's `phase_ends_tick`. Interpolate a recoil from `phase_started_tick`, never from the
first frame that happens to notice the phase.

### Sensing, stopping and admission, reconciled with the real reach

The claws close `|capture_offset| + capture_reach ≈ 14.8` px from the root, so the profile now
senses at the genome's maximum `12` px and **pays for it** at the ordinary `sense_cost` every
tick (`0.0024 e/s` against the old `0.0016`). Acquisition and retention use the pair pass, whose
reach is `sense + both extents ≈ 22.4` px, comfortably past the grasp; a test asserts that
relation rather than leaving it implicit. A stalk closes only while the grasp is still ahead of
the prey: a prey already inside the reach envelope is **not** approached further, and a windup is
admitted only within `capture_reach · scale + prey extent + strike_speed · strike_seconds` — the
gap the paid strike can actually close. Capture is still evaluated **once, at the end of the paid
strike**; a windup never kills.

### Both arms of an experiment

```rust
let profile = FixedHunterProfile::lanternjaw_trial(world.config());   // or .facultative() / .without_attacks()
let target = HunterTarget { face: 4, u: 32.0, v: 32.0 };
let receipt = world.start_hunter_trial(profile.clone(), target)?;     // hunter arm
let control = other.deposit_hunter_budget_control(profile, target)?;  // budget-matched arm
```

Both validate everything before touching the world, refuse a repeat, refuse the other arm's
world, and return the amounts actually booked. Neither resets the world's opening history: the
import is booked in the extension, so `mass_residual()` is unmoved by the initialization itself
(asserted in debug).

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

## Schema 11, and what happens to schema 10

`SCHEMA_VERSION` is now **11**. Schema 10 appended `WorldState.hunters`; schema 11 changed the
*shape* of that extension — the measured capture effector, the ingestion mouth, the visual query
extent, the body-scale mapping, and each member's `entered_from` and `episode`.

`snapshot/v10.rs` freezes the schema 10 world, including its own copies of the version 1 profile
and member, and is the one migration in this crate that can refuse:

- an **empty** schema 10 extension migrates exactly, like 9, 8 and 7 before it;
- a schema 10 payload carrying an **active trial** is `SnapshotError::Invalid` naming why — its
  profile cannot be given a capture geometry and a scale mapping it never had without silently
  changing what a running experiment meant. Re-create the trial from its recorded recipe.

`v10::project` returns `Option`: a schema 11 world with a trial running has no honest schema 10
image. `ecology_hash` is still the schema 7 projection, so care/no-care comparisons are
unaffected, and the genuine 9/8/7 continuations are byte-identical as before.

## Trial parameters (placeholders, not balance)

Founder genome `size 2, reserve 2, mouth 1, speed 1, sense 12, metabolism 0.5, depth 0.85,
swim 0.1, diet 0`, form 4, role `Lanternjaw`. At the default config that decodes to
`S_adult = 2`, `R_max = 4`, `E_max = 4`, `v_max ~ 0.2523 px/s`, maintenance `0.0025` per
structural unit per second, and the derived founder inventory is `S = 2, R = 2, E = 3` —
**4 material and 7 energy**, derived from the config, never hardcoded.

Geometry: capture effector `(13.279411764705882, 1.1)` with a `1.5` px tolerance, ingestion
mouth `(9.6, 0)`, physical extent `9`, visual query extent `16`, scale `sqrt(S / S_adult)`
floored at `0.2`. Both offsets are **Fable's authored** `Lanternjaw::effectors(1.0)`, recomputed
in `hunter::CAPTURE_OFFSET_BODY` / `INGESTION_OFFSET_BODY` from the same source expressions
(`head_dx = -0.3·(1 − 13/17) + 1.1·clamp((13 − 9)/8, 0, 1)`, near claw
`(12.3 + head_dx + 0.5, 0.6 + 0.5)`, mouth `(8 + 1.1 + 0.5, 0)`), read from
`crates/cubarium/src/lanternjaw.rs` read-only. A unit test re-evaluates those expressions rather
than comparing copied decimals, so a moved claw fails the build instead of drifting; the arms
are never shortened to fit core. The `1.5` px tolerance and the `16` px query extent remain
**trial values** the art worker must confirm against the drawn rig.

Behaviour: perch above 65 % reserve, hunt below 35 %; prey `0.15 <= S <= 0.75 · S_hunter` whose
whole inventory fits the 4 m gut; 8 s stalk timeout; 0.6 s windup (no capture, and the hunter
holds position while cocking); 1 s strike at up to 1 px/s costing 0.08 e, charged in full at
entry; capture `clamp(0.65 · S_h/(S_h + S_p), 0.1, 0.75)` evaluated once at strike end; 5 s
recovery after a failed attempt; handling 0.002 e/s; digestion 0.1 m/s; 20 s pause after a meal,
starting in the tick the gut empties; escape up to 2x the prey's own maximum speed within
240 deg/s, both limited by the movement energy it has. Reproduction: full adult structure,
1200 s age, 80 % reserve, 75 % energy, no gut, no hunt, 1800 s since the last birth, 120 s
gestation, juvenile growth 0.002 m/s.

## What a tick actually does now

All of it is skipped whole when the member list is empty, so a world without hunters runs the
pre-hunter tick operation for operation, with no additional draws. The proof is the byte-exact
continuation below.

1. **Phases (after the ordinary decisions, before movement).** Each member runs its own local
   machine on a copy of its record: drop a target that died, had its slot reused, became a
   hunter, grew out of the window or stopped fitting the gut; drop a stalk or a windup that
   lost local sensing of its target; expire timed phases; perch when satiated or carrying.
   A hungry hunter with attacks enabled takes the **nearest eligible prey its own neighbour
   list already holds** — sorted by `(distance, id)`, never a global scan. It closes only while
   the grasp is still ahead of the prey, and winds up (0.6 s, no capture, holding position)
   when the prey is within the gap a paid strike can close, measured from the root in the
   renderer's body basis. At the end of the windup it
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
   the full ID as the tiebreak, and resolved once. Contact is re-evaluated from the hunter
   **root**, and a capture additionally requires a grasp centre the renderer could actually
   draw — an off-rim or vertex-ambiguous grasp is `GraspUnmapped`, paid and refused. The whole
   settlement is recorded as `ContactEvidence` **before** anything is removed. At most one
   hunter claims each prey; a contender whose target was
   claimed is told so and keeps its payment. A claimed prey is removed exactly once, with one
   ordinary `LifeEvent::Death` carrying `DeathCause::Predation`, and its whole body **and
   escrow** move into the gut — an internal transfer with no source ledger and no detritus
   cap. The three persisted natural-death counters never move.
5. **Handling and digestion (before physiology).** The handling cost is paid first and in
   full, or nothing is digested that tick. A portion leaves the gut with exactly the energy it
   was carrying, stores `eta_m · min(1, rho/e_r) · q` as reserve, rejects the rest as
   energy-free litter in the hunter's own cell, and sends the spare energy to the battery or
   to heat. Reserve headroom shrinks the **portion**, not the assimilation. A finished meal
   ends exactly empty **and the recovery pause starts in that same tick**, so a member is never
   left handling nothing; a satiated hunter keeps its meal, counted and checkpointed, with no
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
cargo test -p cubarium-core --offline          # 270 passed, 0 failed, 2 ignored
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

### `tests/hunter.rs`, 27 deterministic tests

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
- **Geometry.** The grasp reaches **across a seam** onto a face the body is not on and captures
  there, and a reach aimed **past the open rim** captures nothing and publishes no centre —
  the regression that replaced the earlier reflected-capture fixture, which proved a defect
  rather than desired geometry.
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
- **Non-finite scale bounds.** NaN and both infinities on `body_scale_min`,
  `body_scale_exponent` and `escape_speed_multiple` are refused by the profile, by both
  initializers (which change nothing), by state validation and by the decoder — and three
  legitimate points of the range still pass.
- **Decode hardening.** Thirteen crafted extensions are refused by name: a member that is not
  a live organism, duplicates, a gut past capacity, energy in an empty gut, handling nothing,
  stalking nobody, a timed phase with no duration, an untimed phase that stores one, a hunter
  aiming at itself, members without a profile, a control world holding a hunter, a negative
  import, and an unknown profile version.

### `tests/hunter_geometry.rs`, 8 tests on the corrected contact

- **Every published grasp centre round-trips through the root chart.** A sweep over five faces,
  twenty-five positions each, eight headings and both offsets: where a centre is published, the
  root's own shortest image of it is the body coordinate it was built from (within `GRASP_EPS`)
  and its sweep neither reflected, fell back nor resolved a tie; where nothing is published, the
  sweep explains why. The sweep exercises hundreds of publications and real refusals at rims and
  vertices, so it is a property of the whole chart, not of one fixture.
- **Contact is decided at the grasp, not at the thorax**, on an interior point and across an
  ordinary seam: a prey at the published centre is captured, and a prey at the old six-pixel
  placeholder never is.
- **A juvenile grasps at its own published scale.** A member shrunk to a funded child's
  structure has `body_scale = sqrt(S/S_adult)`, every published number is that one scale applied
  to the profile, it captures at its own claws, and a prey at the *adult* reach is out of its
  grasp entirely.
- **Capture metadata is the settlement.** The evidence measures its own reported pairing when
  recomputed from its own numbers, the grasp and mouth are distinct points, and the `Capture`
  and its paid `Attempt` carry one `attack_counter` key that equals the member's counter and its
  `episode`.
- **Unpaid refusals carry no paid key**, so they can never alias a paid attempt, and a stale
  target carries no prey position.
- **Phase timing and episode identity survive a restart.** The view's timing is the member's
  timing; a checkpoint taken mid-strike reloads to the same hash and the same member record, the
  resumed run reproduces the event stream key for key, and the phase opened by settlement starts
  at the settlement tick in both worlds — checked at the settlement, not at the end of the run.
- **A settled phase starts at the settlement tick and runs its whole span.** A capture opens
  `Handling` exactly at its record's tick, from a fully extended strike; a paid miss opens
  `Recovering` at its attempt tick, is still recovering when observed at its advertised end
  boundary, and hands over to a contiguous next phase one step later.
- **The pause after a meal opens at the tick the gut empties**, for the profile's whole span.

### `tests/hunter_migration.rs`, 6 tests

The byte-exact schema 9 continuation above, the schema 8 and 7 projections, the inert
never-opted-in world, plus: a schema 10 snapshot **without** a trial migrates exactly, and one
**with** any hunter history — a running trial, a budget-matched control deposit, or an extinct
lineage's counters — is refused by name, while a running schema 11 trial refuses to project
backwards.

### `tests/hunter_reproduction.rs`, 7 tests on the transactions

- **Funded, then born.** The debit equals the escrow plus its build heat, in material and in
  energy; the build heat is the world's own `build_cost · S`; the escrow is the world's own child
  fractions; the child's actual opening inventory is the escrow's; `birth_heat = e_r · S`; and the
  `Offspring` and `LifeEvent::Birth` records reconcile one-for-one. Exactly one record closes the
  key.
- **Refused at the cap.** Every unit goes back to the parent, the parent's post-step stocks match
  the record, no child exists, and no `Born` or `Miscarried` is emitted for that key.
- **Funding blocked by the cap** is a `NotFunded` non-transaction: no escrow, no key, no funding
  record, and the world's own `cap_rejections_total` agrees.
- **Miscarried at the parent's death.** The escrow's own material and energy only, the cap split
  accounted, the escrow strictly smaller than the corpse, and — in a quiet world — the litter
  gaining exactly body plus escrow. The member's `Death` record carries its gut terms separately.
- **Funded and lost in one tick.** Both records, same key, same tick, cause `Age`.
- **A saved gestation replays the same transaction stream**, event for event, beside identical
  state hashes.
- **`tick()` and `hunter()` agree with every variant**, and the stream stays in commit order.

Beside them, in `tests/hunter.rs`: **observations do not move a profile-3 hunter world.** Two
fixed scenarios — one that hunts, captures and digests, one that funds an escrow and gives birth
— are pinned by full-state hash recorded from `9eacb7e`, *before* any of this evidence code
existed. Both hashes are unchanged, which is the proof that adding these records changed nothing
root's twelve-seed screen is measuring.

## Open, and not claimed

- **No ecological claim at all.** These are unit and integration tests of mechanism, not
  evidence about balance. Long-run prey margins are *not* validated: the twelve-hour comparisons
  retain total population but lose skimmers and founder diversity, so a green suite is not a
  reason to put this lineage anywhere near the live world.
- **The paired experiment has not been run, and this package does not run it.** Root owns the
  harness and the runs: **six arms** (untouched, budget control, specialist off/on, facultative
  off/on) across **all twelve seeds** at two hours, per
  `hunter-experiment-contract-2026-09-13.md`. Root's harness `5f27172` and its smokes exist; the
  biological screen was waiting on exactly the capture metadata and geometry this package now
  delivers.
- **The art confirmation is still owed.** `capture_offset_body` is now Fable's *authored*
  `effectors(1.0).near_claw`, recomputed from the same source expressions — but it has still not
  been measured against a rendered frame; the `1.5` px tolerance and the `16` px query extent
  are trial values. Debug contact overlays and the seam/vertex/rim visual comparison belong to
  the art package, which owns `lanternjaw.rs`, `multipart.rs` and the whole-rig scaling this
  core hands `body_scale` to.
- **The admitted scale ranges do not yet meet.** Core admits `body_scale_min = 0.2`, and a
  perfectly valid config (`child_structure_fraction = 0.1`) yields `0.3162…`; Fable's
  `Lanternjaw::draw_living` currently admits `SCALE_MIN..=SCALE_MAX = 0.5..=1.0` and panics
  outside it, and is extending that range. Core tests the small end rather than clamping to the
  renderer's: clamping only one side would separate the drawn claws from the contact geometry.
  Until the art range covers `0.2`, a world whose children are born that small can be simulated
  but not drawn — root should keep the default `child_structure_fraction = 0.4` (scale `0.632`)
  for the screen.
- **Untuned trial numbers with a known demand.** At the founder defaults, maintenance alone is
  about 18 e/hour before sensing and movement, and sensing now costs `0.0024 e/s`; how much prey
  turnover that actually requires is a measurement nobody has made yet.
- **Events are not history.** The hunter event stream is transient, including the new
  transaction records. Root must journal what it wants to keep: a reloaded snapshot replays the
  same records going forward, but does not reproduce the ones drained before the checkpoint.
- **One host change is outstanding and is root's.** `hunter_compare.rs:377` matches
  `HunterEvent` exhaustively to read a tick and no longer compiles; `event.tick()` replaces the
  whole match. Until root adapts it, `cargo check --workspace` fails there and only there —
  `cargo test -p cubarium-core --offline` is green.
- **Nothing here measures reproduction rates.** These records say what each transaction moved;
  how often a lineage funds one, and whether it replaces itself, is what root's screen is for.

## Remaining gates before this lineage goes anywhere

1. Root's six-arm, twelve-seed two-hour screen completes with its accounting gates passing, and
   its capture/recovery metrics come out of the new `ContactEvidence` rather than an
   approximation from the previous tick.
2. Fable confirms `capture_center` tracks the drawn claw at rest, full coil, settlement and
   recoil, at adult and juvenile scale, and maps `HunterRole::Lanternjaw` to the body; the
   renderer scales the whole rig by the published `body_scale`.
3. Wrysk's explicit authorization for any live introduction or default change. Nothing here
   alters a default: an untouched world has no profile, no members and no imports, and steps
   exactly as the pre-hunter build did — the byte-exact schema 9 continuation still proves it.

## Deviations, and why

- **Schema 11, and a refusal.** The geometry correction changed the persisted profile and member
  shape. Rather than smuggle a new meaning into the same wire version, the schema is bumped, the
  old shape is frozen in `snapshot/v10.rs`, empty schema 10 worlds migrate exactly and any
  non-empty schema 10 extension is refused by name.
- **`PROFILE_VERSION 3` for a value, not a shape.** Adopting Fable's authored claw moved a
  frozen experimental constant by `0.062368` px. The wire shape is identical, so nothing forced
  a version bump — but a saved version 2 trial would then silently mean something new, so it is
  refused instead. Provenance for the new value lives in `hunter::CAPTURE_OFFSET_BODY` and its
  test, which re-evaluates the art's own expressions.
- **Two draws per paid attempt, not one.** The plan asks for "one draw per paid attempt" and
  also for a "dedicated seeded draw" for contested-claim priority. Deriving both from one draw
  would correlate a hunter's claim priority with its own capture roll, so priority and capture
  are separate counters in the same stream (`2·n` and `2·n + 1`). Nothing else consumes a draw.
- **Sensing raised in the world, not in the presenter.** Eight pixels could not acquire prey the
  claws could grasp. The genome now senses at its own maximum, 12 px, and pays the ordinary
  sense cost for it every tick.
- **`HunterView.mouth` removed rather than kept as a lie.** It reported a reflected point as if
  it were visible. The replacement names capture and ingestion separately and admits `None`.
- **A refused birth waits one gestation**, not the full reproduction interval: long enough that
  a world at its cap cannot retry every tick, short enough that a refusal is not a de facto
  half-hour ban. The plan asks only for "a retry/recovery interval".
- **The persisted transition origin is two fields, not an animation state.** `entered_from` and
  `episode` are semantic facts the tick already knew; the adapter reads them instead of guessing
  a recoil's origin from durations. No pose, no timeline and no generic animation state entered
  the snapshot.

## Corrections to earlier records

- The commit message of `327862c` says "261 passed"; the actual count at that commit was **260**
  (`139 + 5 + 13 + 6 + 6 + 7 + 26 + 8 + 6 + 2 + 32 + 2 + 8`). The tree was green either way; the
  number in the message is one too high, and is corrected here rather than by rewriting a commit
  other workers may already have built on.
