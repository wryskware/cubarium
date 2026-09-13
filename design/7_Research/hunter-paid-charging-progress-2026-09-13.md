---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Paid hunter charging: core policy and recipe implementation progress

Implementation record for `astra-hunter-paid-charging-proposal-2026-09-13.md` (`0ae1022`).
Evidence, not decisions.

**Nothing here is a canonical acceptance, a balance claim, or a live introduction.** No cohort
was launched. That charging can open a stock gate in a hand-built fixture is a mechanism fact
about that fixture; it says nothing about whether a lineage is viable, and a synthetic birth is
not a lineage. The live build, the shim, `state/` and the two terminal experiments
(`captures/hunter-reserve-baseline-two-hour-b547ad0` and its candidate counterpart) were not
read from as inputs, not modified, and not stopped.

## Commits

| Commit | Scope |
| --- | --- |
| `b4dfd1b` | Core: `OxidationPolicy`, semantic profile version 4, the member threshold resolution, the bounded `ChargingDiagnostics`, three genuine pre-change fixtures and 16 focused tests |
| `83e562e` | Harness: the `reserve-targets-charge80-v1` recipe, resolved policy in the manifest and per-arm openings, per-arm oxidation diagnostics, a second strict reduction contract with its own tests, and an art-adapter capability test |

Both were taken under `flock /tmp/cubarium-shared-care/git.lock`. Nothing was pushed.

Untouched in both: `crates/cubarium/src/art_present.rs`, `art.rs` and the art test files (Fable's
tall-plant wind work, in progress), `src/sink/web/index.html`, all care code and defaults,
`.vscode/`, `.mcp.json`, `.claude/`, `graft/`, `.gitignore`, `AGENTS.md`, `art/` and `assets/`.

## The policy ABI

### Core

```rust
pub const PROFILE_VERSION: u32 = 3;              // unchanged; still what lanternjaw_trial writes
pub const PROFILE_VERSION_CHARGE80: u32 = 4;
pub const SUPPORTED_PROFILE_VERSIONS: [u32; 2] = [3, 4];
pub const CHARGE80_OXIDATION_THRESHOLD: f64 = 0.80;

pub enum OxidationPolicy { Configured, Fixed(f64) }
impl OxidationPolicy {
    pub fn threshold(self, config: &OrganismConfig) -> f64;
    pub fn as_str(self) -> &'static str;   // "configured-world-threshold" | "fixed-member-threshold"
}
impl FixedHunterProfile {
    pub fn charge80(self) -> FixedHunterProfile;                     // version := 4, nothing else
    pub fn oxidation_policy(&self) -> OxidationPolicy;
    pub fn oxidation_threshold(&self, config: &OrganismConfig) -> f64;
}
impl World {
    pub fn charging_diagnostics(&self) -> ChargingDiagnostics;
    pub fn member_oxidation_threshold(&self) -> f64;
}
pub struct ChargingDiagnostics {          // Copy, Default, transient — never persisted or hashed
    pub extra_transactions: u64,
    pub extra_reserve_burned: f64,
    pub extra_energy_gained: f64,
    pub extra_heat: f64,
}
```

`validate` accepts exactly `{3, 4}` and refuses anything else by name
(`hunter profile version N is not one of [3, 4]`).

### What version 4 changes

One line of the step: the threshold in `energy < threshold · E_max`, for **authoritative hunter
members only**, resolved through `profile.oxidation_threshold(org_cfg)`. Version 3 resolves to
the world's own `organism.oxidation_threshold` — the same expression, bit for bit, so a version 3
member is not "the same to within rounding". Ordinary organisms never consult a profile at all.

The conversion block is untouched:

```text
burn = min(oxidation_rate · DT, actual reserve)
reserve -= burn;  local N += burn
released = reserve_energy_density · burn
gain = min(released · oxidation_efficiency, battery headroom)
energy += gain;  heat += released - gain
```

Unchanged: oxidation rate and efficiency, intake, gut capacity, digestion and handling,
maintenance, sensing, movement and strike payment, body and geometry, steering and targeting,
reserve targets, founder stocks, growth, offspring payment, every reproductive gate, gestation,
mortality, care, RNG and capacity. No free energy, no lowered gate, no reserve floor, no age
condition, no strike-admission change. Charging *spends* reserve, so it moves the reserve gate
the wrong way; a member with nothing to burn charges nothing and still pays its upkeep.

### The diagnostics, and their exact scope

A transaction is counted when, and only when, a member's resolved threshold was **above** the
world's configured one *and* its energy was at or above the configured one — the exact
transactions a version 3 member would not have performed in the same world at the same instant.
Ordinary physiology below the configured threshold is not counted by either policy.

The condition is decided **before** the transaction mutates anything, so it is the test a
configured-threshold member would actually have failed, not a subtraction reconstructed
afterwards from rounded values.

Time scope: from the moment the `World` value was constructed (`World::new` or
`World::from_state`) to now. Four running scalars and a counter, written only inside a branch the
step already takes. No per-tick history, no RNG draw, no world state read or written for
measurement. They are **not** in `WorldState`, so they cannot reach a snapshot, a hash or a
schema — and a reload starts them at zero, which makes them a property of one run rather than of
a world. The harness records them per arm, where one arm is one run.

### Persistence

Version 4 shares version 3's serialized shape exactly — asserted both as `postcard` length
equality of the profile and as payload-length equality of two whole worlds. **No new snapshot
schema and no new frozen mirror is needed**, and none was added: schema 12 remains the care-dose
schema. An old reader decodes those bytes and then refuses them at the semantic check.

The proposal asked to report rather than invent if the same-shape scheme proved unsound. It did
not: `FixedHunterProfile::validate` is reached by `HunterState::validate` →
`WorldState::validate` → `decode_snapshot`, `World::new` and `World::from_state`, so there is no
door into a world that skips it. The limit is stated in the source: this suits **one frozen
experimental constant**. An extensible matrix of tunable policies would need an explicitly
persisted field and an honest frozen old-shape migration, not more combinations encoded in
version numbers.

## The recipe

`--profile reserve-targets-charge80-v1` is `reserve-targets-v1`'s fixed seek .80 / perch .90
background with the semantic version raised 3 → 4 and **nothing else** — asserted over the whole
profile struct, on all six arms. The two attack-disabled controls take the policy too: they
exist to expose starvation and intake without predation, and a control quietly running the other
policy would not be a control for this experiment.

The manifest records, from the cohort's **own** configs (with every opening required to agree):

```json
"profile_recipe": "reserve-targets-charge80-v1",
"member_oxidation_policy": "fixed-member-threshold",
"member_oxidation_threshold": 0.8,
"world_oxidation_threshold": 0.5,
"profile_recipe_scope": "… version 3 raised to 4 and nothing else; version 4's one meaning is a fixed 0.80 E_max …",
"oxidation_observer": "charging_above_reference; per-arm running totals …"
```

Each arm's `opening.json` records two separate facts — `recipe_oxidation_policy` /
`recipe_oxidation_threshold` (what the recipe carries) and `world_member_oxidation_threshold`
(what that arm's world resolved). They differ for the untouched arm, which installs no profile
at all. Each arm's `summary.json` carries `oxidation.charging_above_reference` with its scope
stated in the record.

`scripts/compare-hunter-recipes.mjs` gains a **second** contract, not a loosened first one:

* `compare(baseline, reserve-targets-v1)` — the terminal study, schema 11, unchanged. Re-run
  against the two frozen two-hour experiments: `artifact_checks_passed: true`, 12 seeds, 144000
  ticks, exit 0.
* `compareCharge(reserve-targets-v1, reserve-targets-charge80-v1)` — schema 12; requires the
  profile diff to be the version alone on a confirmed .80/.90 background, the recorded policy to
  match the recipe, the resolved threshold to match the policy, the background recipe to report
  **zero** extra oxidation, and the diagnostics to be internally consistent in shape and sign —
  never a value, which is the experiment's result and not the script's business. Only the
  untouched arm is required to be identical; every arm carrying a member, controls included, may
  legitimately diverge.

## Evidence

### Genuine pre-change fixtures

Three schema 12 / profile version 3 snapshots written by the **pre-change release build** in an
isolated `git archive 0ae1022` tree at `/tmp/cubarium-pre-charge-fDU26F`, by a generator that
asserts `SCHEMA_VERSION == 12` and `PROFILE_VERSION == 3` before writing anything. Release build,
not debug; full paths, SHA256s and payload hashes in
`crates/cubarium-core/tests/fixtures/hunter-v3-charge-provenance.md`; generator source committed
verbatim beside them. That isolated tree is mine for this package — it is **not** the frozen
schema 11 reserve recipe at `/tmp/cubarium-reserve-recipe-QkCViu`, which was not touched.

| Fixture | Tick | Founder battery | Role |
| --- | ---: | --- | --- |
| `hunter-v3-charge-window.cubw` | 1400 | 0.623255 of `E_max` | strictly inside (0.50, 0.80): the band where the policies disagree |
| `hunter-v3-charge-active.cubw` | 5570 | 0.475850 | below 0.50 and actually oxidizing, 0.1 m of reserve already burned |
| `hunter-v3-charge-active-plus600.cubw` | 6170 | 0.448846 | the continuation target |

Loading the active fixture into the policy build, stepping 600 ticks and re-encoding reproduces
the pre-change binary's payload **byte for byte**, with zero extra transactions. From the window
fixture's exact bytes, 100 ticks under version 3 burn nothing at all and record nothing; under
version 4 they charge on every one of those 100 ticks.

### Old-reader refusal, from an actual frozen executable

Pre-change host binary from the same isolated tree, `cargo build --release -p cubarium --bin
cubarium`, SHA256 `b32db44484077bcccced02929c694c71c2737f55b02189fc23b30406a34ad309`.

```text
$ <pre-change cubarium> run --sink none --speed 0 --seconds 1 --state <v4 world>
cubarium: skipping …/world-100.cubw: Invalid("hunter profile version 4 is not 3")
cubarium: … This is a damaged world, not an empty directory, so no new world is created here.
exit 1
```

It refused **by name**, did not resume the experiment as version 3, and created nothing. The same
binary resumed a version 3 world written by the *new* build normally (exit 0, tick 100 → 120),
which is the other half of the claim: the shape really is unchanged, and only the semantics are
gated. The version 4 snapshot for that check came from a throwaway core example, deleted after
use and reproduced here for provenance:

```rust
// crates/cubarium-core/examples/tmp_v4_snapshot.rs — temporary, not committed
let mut world = World::new(WorldConfig::default())?;         // step to tick 100
let profile = FixedHunterProfile::lanternjaw_trial(world.config()).charge80();
world.start_hunter_trial(profile, HunterTarget { face: 4, u: 32.0, v: 32.0 })?;
std::fs::write(dir.join("world-100.cubw"), encode_snapshot(&world.state, "v4-policy"))?;
```

Run as `cargo run -q -p cubarium-core --example tmp_v4_snapshot` — a **debug** build, unlike the
fixtures above. That is sound for this check: the question is whether an old reader accepts the
bytes, and the bytes carry the same profile either way.

### Tests

All foreground, all run to terminal completion.

| Command | Result |
| --- | --- |
| `cargo test -p cubarium-core` | **307 passed, 0 failed, exit 0** |
| `cargo test -p cubarium --lib --test hunter_charge_capability --test hunter_present --test lanternjaw{,_cost,_living,_scale} --test hunter_observers --test care_replay --example hunter_compare` | **405 passed, 0 failed, exit 0** |
| `node scripts/compare-hunter-recipes.test.mjs` | **11 passed, 0 failed, exit 0** |
| `node scripts/compare-hunter-recipes.mjs <baseline> <candidate>` (the two terminal two-hour experiments) | `artifact_checks_passed: true`, 12 seeds, exit 0 |
| `cargo test --workspace` in a clean worktree at `83e562e` | **1075 passed, 0 failed, exit 0** |
| `cargo clippy -p cubarium-core --all-targets` | clean |
| `cargo clippy -p cubarium --all-targets` | no findings in any file this package added or changed |

The workspace suite was run in a detached worktree at the committed hash rather than in the
shared tree, because the shared tree currently carries Fable's in-progress tall-plant wind edits
and one of their `art_wind` tests does not pass there. That failure is in
`crates/cubarium/tests/art_wind.rs` against `art_present.rs`/`art.rs`, none of which this package
touches; at the committed hash the same test passes (18/18). The worktree was removed afterwards.

The 16 new core tests, by the proposal's own list: activation strictly inside (0.50, 0.80) and no
activation at or above 0.80 — checked at both exact boundaries in a world with maintenance,
sensing and movement costs set to zero, because an ordinary tick spends the battery *before*
physiology and would otherwise make a strict-inequality test pass for the wrong reason;
membership isolation, with an ordinary organism at 0.6 of `E_max` sitting beside a charging
member and not charging; the burn ceiling, the reserve-to-`N` return checked from the cell, the
`e_r`/efficiency/heat identity, and the headroom cap binding with the excess becoming heat; zero
reserve and a sub-ceiling scrap; ordinary-organism and version 3 identity, including the
byte-for-byte continuation; the version diff over the whole struct and the persisted encoding;
unsupported-version refusal at `validate` and at `decode_snapshot`; a fully paid escrow opened by
charging when stocks allow, priced from the config and paid out of the parent's own stocks, with
the version 3 control failing to reach the gate at all; a reserve-limited member that charges and
still never funds; and mid-charge and mid-gestation resume with identical full state hashes every
tick for 600 ticks and identical hunter and life event streams. Full state hashes throughout,
never the ecology projection, which deliberately drops the hunter extension.

Two host tests ask the published art capability questions without editing any presentation
source: the renderer accepts both versions and returns byte-identical refusals for the same
geometry damage under either, and two worlds differing only in the stored version draw the same
frame byte for byte.

### Authorized short smoke

Release build of the committed harness, `target/release/examples/hunter_compare`, SHA256
`77a466623593e822e6d8f60411e7b8a2de507fef7a8a183c68b5d1333eb6196e`, at `83e562e`. All twelve
seeds, all six arms, **2000 ticks** — a harness smoke, far too short for any biological reading,
and deliberately below the 144000 the reduction script demands. Fresh named output directories;
nothing was overwritten.

| Directory | Recipe | Exit | Seeds | Failures |
| --- | --- | ---: | ---: | --- |
| `captures/charge80-smoke-83e562e-baseline-control` | `baseline` | 0 | 12 | none |
| `captures/charge80-smoke-83e562e-reserve-targets-v1` | `reserve-targets-v1` | 0 | 12 | none |
| `captures/charge80-smoke-83e562e-charge80` | `reserve-targets-charge80-v1` | 0 | 12 | none |

The first directory is named for what it is: I ran it before passing `--profile`, so it is a
baseline run, kept and relabelled rather than deleted or quietly reused.

Summed extra-oxidation diagnostics over all twelve seeds, 2000 ticks:

| Arm | `reserve-targets-v1` | `reserve-targets-charge80-v1` |
| --- | --- | --- |
| untouched (threshold 0.5, no profile) | 0 transactions | 0 transactions |
| budget_control (threshold 0.8, no member) | 0 | 0 |
| specialist_off | 0 | 14325 tx, 7.1625 m burned, 11.46 e gained, 2.865 e heat |
| specialist_on | 0 | 17862 tx, 8.931 m, 14.2896 e, 3.5724 e |
| facultative_off | 0 | 14309 tx, 7.1545 m, 11.4472 e, 2.8618 e |
| facultative_on | 0 | 17854 tx, 8.927 m, 14.2832 e, 3.5708 e |

The background recipe reports zero on every arm by construction, not by luck. The candidate's
totals satisfy the conversion identity exactly — `11.46 = 2 × 7.1625 × 0.8` and
`2.865 = 2 × 7.1625 − 11.46` — and the attack-disabled controls charge, as intended. Running the
new pair check over the smoke's real artifacts: 72 `verifyChargePair` openings and 144
`verifyOxidation` records pass, and the untouched arm is byte-identical across the two recipes on
all twelve seeds.

No offspring in any arm of any smoke run, and 50 / 48 captures in the two member recipes. At 2000
ticks that means nothing either way; it is reported because a silent omission would read as a
result.

## Limits, stated plainly

* **No acceptance and no rollout.** Nothing is enabled by default, nothing was introduced to the
  live world, and no live interaction was run. Version 3 is still the default constructor.
* **No biological claim.** More battery is not success if reserve readiness collapses or every
  founder still starves. Charging may also free reserve headroom for digestion, consume reserve
  a juvenile needed for growth, trigger earlier hunting through the existing reserve thresholds,
  and raise prey burden. Those are consequences to measure in the full paired run, not results.
* **The full cohorts are not launched**, by instruction: root coordinates both frozen matched
  jobs after reviewing this. The smoke's 2000 ticks is not evidence about anything ecological.
* **The reduction script cannot read the smoke.** `loadRun` still requires ≥144000 ticks, which
  is why the contract was exercised against the smoke's artifacts through its exported functions
  rather than through `compareCharge`. `compareCharge` itself has unit tests but has not yet run
  end to end on a complete pair, because no complete pair exists.
* **The diagnostics are per-run, not per-world.** They are transient by design, so a resumed run
  reports only its own share. The harness runs each arm in one process, which is why per-arm
  totals are meaningful there; a reader must not add them across a restart.
* **The escrow fixture is hand-built.** It winds the reproduction *age* clock to zero so one test
  can watch a gestation — the two stock gates that charging must actually open are untouched. It
  demonstrates a mechanism in one fixture and is not a viable lineage.
* **Version 4 is one frozen constant, not a policy framework.** Encoding further policies as
  version numbers would be the wrong next step; the source says so where it matters.
* **Signed strike-admission remains a separate later family** and was not combined with this one.
