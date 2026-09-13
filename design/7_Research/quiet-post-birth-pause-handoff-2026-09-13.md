---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# `post_birth_pause_v1`: core implementation handoff

Implementation record for the first slice of
[the ordinary quiet proposal](astra-ordinary-quiet-experiment-proposal-2026-09-13.md). Evidence,
not decisions.

**Nothing here promotes anything.** The autonomous default is Off and stays Off. Forty ticks is
the proposal's unvalidated candidate duration, not an accepted one. Passing unit and migration
tests establish that the mechanism does what the contract says; they say nothing about quiet
opportunity, viability, offspring recruitment, or permission to change a default. The four-arm
twelve-seed experiment is **not** built here and must not start before this slice is reviewed.

No live state, care journal, viewer, art, README, implementation-plan, hunter recipe or ordinary
physiology tuning was touched.

## Commit

`<this commit>` — core quiet module, narrow controller/world integration, snapshot schema 13 with
a frozen schema 12 mirror, migrations and projections, four genuine pre-quiet fixtures with
provenance, and 26 focused tests.

Astra's independent probes (`crates/cubarium-core/tests/astra_quiet_policy.rs`) and review
(`astra-quiet-policy-review-2026-09-13.md`) landed at `59d66b0`, one commit earlier; that commit
does not build on its own, because it exercises the API this one adds. The five worker-owned
source files hash exactly as that review recorded them.

## The rule, as built

After an **actual paid child insertion** completes boundary `B`, a surviving *ordinary* parent may
hold exactly forty decisions, `B` through `B + 39`, producing completed `Resting` intervals
`B + 1` through `B + 40`. The decision at `B + 40` is ordinary again. The birth's own tick is never
repainted.

The trigger is the successful core commit — the child is in the arena and the parent survived to
see it — not a post-step escrow inference. Nothing starts a pause on a funding, a refund, a cap
refusal, a death, a newborn's own appearance or an observer record.

During a held decision: public mode is genuinely `Resting`; fruit, grazing and scavenging efforts
are zero, so `fed_this_tick` is false; new budding requests are suppressed. Maintenance, sensing,
the actual movement it still performs, growth, oxidation, the death checks and ageing all run in
their existing order at their existing amounts. Hunger and hunger memory keep telling the truth.

## API

```rust
// crates/cubarium-core/src/quiet.rs
pub const QUIET_VERSION: u32 = 1;
pub const POST_BIRTH_PAUSE_TICKS: u64 = 40;
pub const SAFETY_MARGIN_TICKS: u64 = 1;

pub enum QuietPolicy { Off, PostBirthPauseV1 }          // Default::Off; .enabled(), .as_str()
pub enum QuietReason {                                   // .as_str()
    Unaffordable, UnaffordableRemaining, ParentGone, Bounded,
    AlreadyPaused, HunterMember, InvalidInputs, Overflow,
}
pub enum QuietEvent {                                    // transient, drainable
    Begin  { tick, parent, child, end_tick, underlying },
    Refuse { tick, parent, child, reason },
    End    { tick, parent, child, completed_ticks, underlying },
    Abort  { tick, parent, child, completed_ticks, reason },
}
pub struct QuietPause { parent, child, start_tick, end_tick, underlying }  // .holds/.remaining/.completed
pub struct QuietState { version, policy, pauses }
impl QuietState {
    pub fn post_birth_pause_v1() -> QuietState;          // the explicit initializer choice
    pub fn active(&self) -> bool;
    pub fn find(&self, parent: OrganismId) -> Option<&QuietPause>;
    pub fn validate(&self, tick, cap, hunters_active, alive) -> Result<(), String>;
}
pub struct Budget { per_second, growth, oxidation, energy, material }
impl Budget {
    pub fn of(org: &Organism, cfg: &OrganismConfig, horizon_seconds: f64) -> Option<Budget>;
    pub fn affordable(&self, org: &Organism) -> bool;    // read-only, strict inequalities
}
pub fn horizon_seconds(remaining_ticks: u64, dt: f64) -> Option<f64>;
pub struct QuietOverride { underlying: Mode, hold: bool }

// crates/cubarium-core/src/world.rs
pub struct WorldState { /* … */ pub quiet: QuietState }  // schema 13, appended
impl World {
    pub fn quiet(&self) -> &QuietState;
    pub fn drain_quiet_events(&mut self) -> Vec<QuietEvent>;
}

// crates/cubarium-core/src/controller.rs
pub struct Decision { /* … */ pub underlying_mode: Mode }
pub fn decide_quiet(.., quiet: Option<QuietOverride>) -> Decision;   // decide(..) == decide_quiet(.., None)
```

A harness chooses the policy once, before the world is built:

```rust
let mut state = /* the copied opening */;
state.quiet = QuietState::post_birth_pause_v1();
let world = World::from_state(state)?;
```

There is deliberately **no setter on `World`**: the proposal asks for the policy to be chosen
explicitly, snapshotted, and never hot-toggled.

## Affordability

The proposal's formula, evaluated at admission and before **every** remaining held decision, over
`h = (remaining decisions + 1 ordinary tick)` seconds:

```text
Sbound = max(S, Sa)
B = m·Sbound + move_cost·Sbound·(f·vmax) + sense_cost·r     [energy/second]
G = min(growth_rate·h, max(0, Sa − S))                      [material]
Q = oxidation_rate·h                                        [material]
admit/continue only if  E > B·h + build_cost·G  and  R > Q + G
```

It is a **test, not a debit**: nothing is charged, escrowed, reserved or healed, and `Budget::of`
returns `None` rather than bounding with a NaN if any input is not finite and nonnegative. Strict
inequalities — landing on the bound is not covering it. A parent that stops covering it aborts
before that tick's held decision and uses the ordinary controller on that same tick; it may then
eat what its own cell holds, which is the release working, not a rescue.

## No hysteresis latch

The entry retains the parent's **underlying** ordinary mode. Every held tick, `decide_quiet`
computes the ordinary transition from that retained value (not from the imposed `Resting` sitting
in the organism), records it in `Decision::underlying_mode`, and only then imposes `Resting` —
before the turn gate, the effort, the intake selection and the bud gate all read the mode. The
world writes the fresh underlying value back once. Release passes `hold: false`, which substitutes
the underlying mode and imposes nothing, so the parent re-enters ordinary hysteresis from where it
really was. One decision per organism per tick, from the same observation and the same noise pair:
the override consumes no RNG of its own, and a held tick advances the turn counter by exactly the
ordinary four.

## Persistence

Schema 13 **appends** `WorldState.quiet`, outside every legacy nested organism and config payload.
`snapshot::v12` freezes the complete schema 12 layout; schemas 7–12 all migrate with the extension
Off and no retroactive pauses. Off with no entries costs three bytes.

`v12::project` **refuses** an enabled policy or a held pause, and so do `v8`, `v9`, `v10` and `v11`:
an old-format image of a live timer is not behaviourally equivalent. `v7::project` stays total,
because it is the *ecology* projection that deliberately drops care and every extension, and
`ecology_hash` depends on that.

`validate` rejects: a version other than 1, Off carrying entries, an enabled policy beside a hunter
extension, more entries than the world's organism capacity, entries out of ascending parent order
(which also makes a duplicate inexpressible), a parent that is its own child, a parent that is not
alive at its full generational ID, any duration other than forty, a start after the world's tick,
an end the world has already stepped past, and tick arithmetic that would overflow.

`end_tick == tick` is **valid**: the window is complete and the entry is carrying exactly one thing,
the underlying mode the release decision at that boundary must resume from. Astra's probe found
that a snapshot taken there was unloadable under the first draft; the fix is that only a pause the
world has stepped *past* is stale. Releasing early and feeding the imposed `Resting` into
hysteresis would have been a different defect.

**Hardening found along the way.** `postcard::from_bytes` tolerates trailing bytes, and every
schema since 8 has grown by appending — so a schema 13 payload relabelled as schema 12 decoded
cleanly and silently dropped the extension. `decode_snapshot` now requires every frozen mirror to
consume its payload exactly. This closes the same hole for the care dose and the hunter extension.

## Exclusions

Quiet and hunters are not combined in this slice. An enabled world refuses `start_hunter_trial`
and `deposit_hunter_budget_control` by name; a state carrying both is refused by `validate`, and
therefore by `World::new`, `World::from_state` and `decode_snapshot`. Threat and escape
interactions need their own explicit contract.

## Evidence

Genuine pre-quiet fixtures, written by the release build at `1e6d053` in an isolated `git archive`
tree; full provenance, SHA256s and payload hashes in
`crates/cubarium-core/tests/fixtures/quiet-v12-provenance.md`. Both the default **no-care** and
the default **care** trajectories are covered, from the quiet diagnosis's own opening (cohort
seed 1 at tick 144000) stepped to 147000 with 437 real paid births.

Loading either, stepping 600 ticks with the policy Off, and re-encoding the schema 12 projection
reproduces the pre-quiet binary's payload **byte for byte**, with no record published.

| Command | Result |
| --- | --- |
| `cargo test -p cubarium-core` | **347 passed, 0 failed, exit 0** |
| `cargo clippy -p cubarium-core --all-targets` | clean |
| `cargo test --workspace` | 2 pre-existing failures in `art_motion`, reproduced at committed HEAD in a clean worktree (23 passed, 2 failed there too); no file this package touches is involved |

Tests added: 8 in `quiet.rs`, 12 in `tests/quiet_pause.rs`, 6 in `tests/quiet_migration.rs`,
beside Astra's 8 independent probes. They cover a real affordable birth to the exact forty-tick
pause; unchanged paid-child accounting; the effort, intake, bud, memory and RNG rules on a held
tick; early abort; insufficient reserve and insufficient energy; finite and overflow boundaries;
no admission on a dead parent, a refunded or cap-refused birth, or a newborn; generation reuse;
release from the hysteresis band; real bounded seam and rim movement; restart immediately after
the birth, mid-pause and at the release boundary; malformed schema 13 rejection; and genuine
schema 12 migration and continuity including care.

Three test premises were wrong and were corrected rather than worked around: a held parent's
energy can legitimately *rise* (oxidation is ordinary physiology and still runs, so upkeep is
isolated with a fixture that switches oxidation off); an aborting parent can legitimately eat on
its release tick (so the claim is that no material enters from outside); and a stripped parent does
not starve while food is near (so the death path is exercised by age, which no food averts).

## Limitations

* **Forty ticks is unvalidated.** So is the whole candidate. Off remains the default.
* **No experiment.** The four-arm twelve-seed family, its ten-minute screen and its longer
  horizons are out of scope here and must wait for review of this slice.
* **No opportunity evidence.** Nothing here measures how often a pause is actually admitted in a
  real world, which is the proposal's own behavioural screen. A candidate with only one-tick
  entries would not meet the objective, and these tests would not notice.
* **`snapshot::v12` borrows the live nested types**, because this change touched none of them. That
  borrow is guarded by the genuine fixtures rather than assumed; a future nested-shape change must
  freeze it there, and the migration tests will fail if it is forgotten.
* **The transient records are for a harness**, not the display. Normal art may use the actual
  `Resting` mode; development output must still distinguish `satiated`, `post_birth_recovery` and
  `newborn_initial`, which is the reducer's job and not built here.
* **Astra's review predates this commit.** It records the source hashes it read, which match, but
  asks for a rerun against the final frozen package.
