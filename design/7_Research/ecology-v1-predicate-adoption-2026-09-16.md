---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Adopting the reach envelope as the shipped pursuit stopping rule, and the schema bump that keeps it honest

Workstream V of the ecology v1 next steps
([brief](../handoffs/ecology-v1-predicate-adoption-opus-2026-09-16.md)), item 2 of the
reconciled round-4 result: Astra's order is *"adopt the reach-envelope predicate
**separately**, with a production default, a resume regression and `check_motor` in host
seeding"*. The measurement this acts on is
[P's paired note](ecology-v1-apex-predicate-2026-09-16.md); the pattern a transient contract
follows is [T's](ecology-v1-motor-inertial-2026-09-16.md).

This is an adoption, not an experiment. Nothing here re-decides whether the envelope is the
right rule — P measured that — and nothing here selects, tunes or recalibrates.

## What changed for an operator

**The apex now bursts when it has paid for a burst.** Until today a lanternjaw that had spent
0.08 e on a lunge would sit at 0.3 px/s six pixels short of its prey and let it walk away,
because the line that decides "close enough, stop approaching" tested a *forward half-space*
— is the prey ahead of my claws? — where its own comment, and the rest of the file, meant the
*reach envelope* — is the prey **in** my claws? From now on it means the envelope. Held at
the start of a paid burst falls from 408 of 449 attempts to 19 of 502 on the same four seeds.

**Nothing else moved.** No apex constant, no escape speed, no sense radius, no strike
duration, no mating radius, no equation, no `WorldConfig` field, no plant or prey rule. The
proof is in §3: re-running the two selected ecologies at the retained length with
`--pursuit-stop half-space` reproduces **all 24** retained `final_state_hash`es bit for bit,
and the eight rows with no apex in them are identical under both rules. The change reaches
exactly the worlds that have a predator in them, and nothing else.

**The cube must be started fresh.** A world's saved bytes do not say which rule they were
written under, so a world saved yesterday resumed on this build would keep its tick, its
bodies and its pools and quietly start hunting under a different predicate. That is a
migration by another name, so `SCHEMA_VERSION` is now **17** and every schema-16 world is
refused by number. `cubarium run --fresh` starts a schema-17 world; a resume into a directory
of schema-16 snapshots stops with `UnsupportedSchema(16)` and writes nothing.

**It still does not fix the apex.** Every introduced adult starves, in both arms, in every
run. What the correction buys is a predator that closes on prey between its claws and about
12 px away; past that nothing changes. The next term is the turn radius inside the motor
envelope, and it is untouched here.

## 1. What was adopted

| | before 2026-09-16 | shipped now |
| --- | --- | --- |
| `PursuitStop::default()` | `ForwardHalfSpace` | **`ReachEnvelope`** |
| an ordinary `World` | half-space, and the envelope was opt-in | **envelope, without being told**; the half-space is the opt-in |
| `StrikeRecorder`'s rule | follows `PursuitStop::default()` | unchanged — it follows it by construction |
| `StrikeRecord.stop` serde default | `PursuitStop::default()` | **a named function** returning `ForwardHalfSpace` |
| `SCHEMA_VERSION` | 16 | **17**, with no payload change |

The predicate itself is still written in exactly one place,
`ContactMeasure::pursuit_holds`, and `World::set_pursuit_stop` still selects between the two
readings. The adoption is a change of *which one is default*, plus the refusal that makes
that change safe.

### Silence means the rule of its day, everywhere

The interesting engineering in this workstream is not the flip; it is that **four separate
"missing field" defaults had to stop following `PursuitStop::default()`**. Every retained
artifact in the tree was produced under the half-space, and a derived default would have
silently relabelled all of them as envelope runs the moment the default moved — including
the held/delivered readings P's whole note is built on.

| artifact | field | absent means | why not the derived default |
| --- | --- | --- | --- |
| `hunter::StrikeRecord` | `stop` | `forward_half_space` | a record written before the field existed ran the half-space; reading it under today's default would rewrite what the world did |
| `calibrate::StagePlan` | `pursuit_stop` | `forward_half_space` | every retained calibration stage evaluated the half-space |
| `population::PopulationPlan` | `pursuit_stop` | `forward_half_space` | ditto, and this report's *result* depends on the rule (below) |
| `apex_audit::AuditRow` / `AuditReport` | `pursuit_stop` | `forward_half_space` | K's and N's retained rows predate the field; repaired at integration (`46b6d18`), see §6 |

Each is a named function, not `#[serde(default)]`, so the two defaults cannot drift back
together (the fourth was given its function at integration, `46b6d18`).

## 2. Schema 17: the first bump for a change of semantics

Every previous bump appended a field. 17 appends nothing: the payload is byte-for-byte
schema 16's, and `decode_exact`'s length check — the module's usual evidence that reader and
writer agreed — **cannot tell 16 from 17**. The header is the only evidence there is, and the
version check is what does the refusing.

That is worth stating as a rule, because it will come up again: **a transient whose default
changes is a schema bump even when nothing in `WorldState` moves a byte.** The transient is
kept out of `WorldState` on purpose — putting the rule in `WorldConfig` would move
`calibrate::config_hash` for every existing TOML and break the provenance of every retained
row — and the price of that choice is exactly this: the bytes cannot state the rule, so the
number has to.

**What the resume regression does and does not promise.** It promises that a world saved and
resumed runs the **shipped default** on both sides and reproduces the uninterrupted hash. It
does not promise that an explicitly selected non-default predicate survives a snapshot: an
opted-in `ForwardHalfSpace` is an experimental transient, it is not in the saved bytes, and a
resumed world comes back on the shipped rule. A paired arm is a single uninterrupted run by
construction; if one ever needs to be resumed, the rule has to be set again on the resumed
`World`.

## 3. What the cube will show: the selected rows, re-run

The two selected ecologies, at the retained held-out length, under the new default and again
under the rule before it. Search build **`b3ac2ecV`** — the tree at `b3ac2ec`, one release
binary built once and copied out of the shared target directory before any row was run.

```bash
# both arms, 24 trials each, 8 workers
bin/cubarium-search calibrate --stage holdout-{half-space,reach-envelope} \
  --candidates baseline,fast-leaf --seed-set holdout --seeds 4 --arms 0,1,2 \
  --ticks 360000 --sample-every 600 --introduce-tick 6000 \
  --pursuit-stop {half-space,reach-envelope} --workers 8 --wall-seconds 600 \
  --out runs/ecology-v1-predicate-adoption
```

Wall 189.2 s and 186.5 s; 0 trials skipped at the cap.

### The flip is the only change

| check | result |
| --- | --- |
| `--pursuit-stop half-space` against the **retained** `runs/ecology-v1-calibration/holdout` rows | **24 of 24 `final_state_hash` identical**, and 24 of 24 `final_ecology_hash` identical — across a build id change from `e635088fa5c4-dirty` to `b3ac2ecV` |
| the two arms against each other, **arm 0** (no apex) | **8 of 8 identical** |
| the two arms against each other, **arms 1 and 2** | **0 of 16 identical** |

The second and third rows together are the statement: the rule reaches every world with a
predator in it and no world without one.

### The comparison, four held-out seeds per cell, 360,000 ticks

Populations are per-row means over the four seeds; counts are totals over them. `foliage`,
`wood` and `litter` are late-window means (the last 72,000 ticks). Arm 0 is omitted: it is
bit-identical.

| | baseline arm 1 | baseline arm 2 | fast-leaf arm 1 | fast-leaf arm 2 |
| --- | ---: | ---: | ---: | ---: |
| population, mean | 49.0 → **49.9** | 47.8 → **44.0** | 61.3 → **61.5** | 61.2 → **61.5** |
| population, final | 58.0 → 59.0 | 54.5 → **49.5** | 65.0 → 66.8 | 66.5 → 64.3 |
| population, minimum | 17.5 → 16.3 | 22.0 → **14.0** | 24.0 → 24.0 | 24.0 → 24.0 |
| late foliage | 237.6 → 237.7 | 232.2 → **294.4** | 229.9 → 230.9 | 231.5 → 230.8 |
| late wood | 345.4 → 350.3 | 337.1 → **373.4** | 169.2 → 168.9 | 170.7 → 168.6 |
| late litter | 146.5 → 148.4 | 145.2 → **171.7** | 166.4 → 167.0 | 169.0 → 166.6 |
| apex attempts | 108 → 113 | 220 → 231 | 147 → **186** | 229 → **271** |
| **apex captures** | 6 → **11** | 8 → **11** | 8 → **17** | 7 → **10** |
| prey deaths by predation | 6 → 11 | 8 → 11 | 8 → 17 | 7 → 10 |
| apex introduced / dead by the end | 4 / 4 | 8 / 8 | 4 / 4 | 8 / 8 |
| prey forms present, minimum | 2.0 → 2.0 | 2.5 → **2.0** | 3.0 → 3.0 | 3.0 → 3.0 |
| six gates | kept → kept | kept → **G lost** | kept → kept | kept → kept |

Captures across the eight apex rows rise **29 → 49**. Every apex introduced is dead by the
horizon in both arms, in every cell — the adoption changes how a predator hunts, not whether
it survives.

**One gate moves, and it is worth naming rather than burying.** `baseline` arm 2 — two apex
adults in the *unselected* ecology — loses the **G** gate (both prey guilds alive): its
minimum forms present falls from 2.5 to 2.0, its mean population from 47.8 to 44.0 and its
minimum from 22 to 14, while its late foliage *rises* 27 % because there is less mouth on it.
That is a predator that now actually catches things, in the ecology that was **not** selected.
`fast-leaf`, the selected one, keeps all three forms and all six gates in every arm and every
seed, and its populations move by under half a body. This is a description of the two
configurations as they are, not evidence that either should be re-selected; selection is not
this workstream's to redo.

### The apex itself, 180,000 ticks with the ledger and the strike record on

The calibration rows carry no per-member lifetime or death cause — that is the audit's
product, not the screen's — so P's own instrument was re-run on exactly the two selected
configurations, four held-out seeds, two adults introduced at tick 6,000.

```bash
bin/cubarium-search apex-audit \
  --config runs/ecology-v1-calibration/selected/{baseline,fast-leaf}.toml \
  --seeds 4 --apex 2 --ticks 180000 --introduce-tick 6000 \
  --pursuit-stop {half-space,reach-envelope} --workers 8 \
  --out runs/ecology-v1-predicate-adoption/audit-{half-space,reach-envelope}.json
```

Wall 29.5 s and 29.7 s.

| over 16 lives | `half-space` | `reach-envelope` (shipped) |
| --- | ---: | ---: |
| paid attempts | 449 | 502 |
| **held at the burst's start** | **408 (90.9 %)** | **19 (3.8 %)** |
| delivered | 25 | **469** |
| contacts (`resolved_in_reach`) | 46 | 46 |
| **captures** | **15 (0.94 / life)** | **21 (1.31 / life)** |
| gut material per life (m) | 0.716 | **1.230** |
| `OutOfReach` / `Missed` / `GraspUnmapped` | 402 / 31 / 0 | 456 / 23 / **2** |
| lifetime mean / median / max (ticks) | 11,982 / 11,955 / 14,101 | 11,949 / 10,532 / **23,201** |
| **death cause** | `Starvation` 16/16 | `Starvation` 16/16 |
| ready-together ticks | 0 | 0 |
| prey taken | 15 | 21 |
| prey alive at the end (8 runs) | 441 | 435 |

Captures by the gap the attempt began at, `[lo, hi)` in px of `effector_distance`:

| initial gap | `half-space` n / contacts / captures | `reach-envelope` n / contacts / captures |
| --- | ---: | ---: |
| 0–4 px | 55 / 25 / 8 | 15 / 11 / 5 |
| 4–8 px | 110 / 21 / 7 | 94 / 30 / **13** |
| **8–12 px** | 95 / **0** / **0** | 193 / **5** / **3** |
| 12–16 px | 81 / 0 / 0 | 136 / 0 / 0 |
| 16 px and beyond | 108 / 0 / 0 | 64 / 0 / 0 |

**These are P's four-seed pair, reproduced to the attempt** on a later tree (P's build `d19150ea6ee7`, this one `b3ac2ecV`):
held 408 of 449, captures 15 → 21, contacts flat at 46 against 46, the oldest member 23,201
ticks. P read that flat contact count honestly as the limb its four-seed pair does not
confirm, and nothing here changes that reading — the adoption rests on P's eight-seed
widening and its out-of-sample replicate, not on this run. What this run adds is that the
build being adopted produces the numbers the decision was made on.

## 4. The layouts hold no hunter, so the trainer is untouched

The brief asked for this as a finding, with a stop condition. Checked, and now fixed by a
test (`no_training_or_holdout_layout_founds_a_hunter`):

> Every one of the 4 training and 8 held-out layouts clears `founders.kinds`, sets
> `founders.count = 0` and places exactly one grazer through `World::found_training_animal`.
> None declares an apex profile; each built world holds exactly one body. **No episode world
> holds a hunter**, so the hunt-intent pass the pursuit predicate lives in is never reached.

Three consequences, and they are the reason this workstream touched the ES code as little as
it did.

1. **A trained policy is predicate-independent.** The same weights score identically under
   either rule, because neither rule can execute in an episode.
2. **`Protocol` and `PolicyFile` gain no field.** One there would move every protocol hash
   for a contract no episode can observe. `Protocol::default().hash()` is still
   `0x65c51e05060f0d5a`, pinned by a test, and "pursuit" does not appear in a serialised
   protocol.
3. **`es-evaluate` gains no `--pursuit-stop`.** A switch there would name a difference it
   cannot make. (It was built, then removed on Astra's review — correctly: an inert control is
   worse than an absent one, because a reader believes it.) `es-population` **does** have the
   switch, because its arms above zero introduce a real apex cohort.

If a layout ever founds a hunter, that test fails and says the field is owed.

## 5. The host contract for the motor

`seed_neural_animals` now calls `file.check_motor(world.motor_model())` immediately after
`check_ecology`, refused by name.

**This is the contract existing, not a behaviour change.** The host never calls
`World::set_motor_model`, so `world.motor_model()` is always the shipped `Sweep`; no world the
host builds moves because of this line, and the only thing the check can ever do is refuse a
policy trained under `inertial`. Two tests fix it: a policy naming `inertial` is refused with
both contracts in the message and writes no world, and a policy file with **no** `motor` key
seeds normally — missing means `sweep`, because there was exactly one contract when such files
were written. (Unlike the ecology, where `None` is genuinely unknown and is refused.)

## 6. What this workstream could not do, and what was owed

> **Done at integration (Fable, `46b6d18`), after Astra's re-check asked this note to say
> so:** `AuditRow::pursuit_stop` and `AuditReport::pursuit_stop` now read a missing field
> as the half-space through the same kind of named function (`rule_before_the_adoption`
> in `apex_audit.rs`), with a test that strips the field from a serialised row and reads
> it back; `Arm::stop`'s doc and `parse_pursuit_stop`'s refusal message name the envelope
> as the shipped rule. The two bullets below are kept as the record of what this
> workstream handed over.

- **`apex_audit::AuditRow::pursuit_stop` and `AuditReport::pursuit_stop` are still
  `#[serde(default)]`.** With the default flipped they now read a rule-less retained row — K's
  and N's, written before P added the field — as `reach_envelope`, which is wrong: those rows
  ran the half-space. The repair is one line each, the same named function this note's other
  three artifacts use. `crates/cubarium-search/src/apex_audit.rs` is workstream U's file for
  the moment and was not touched; **this is owed at integration.**
- **The same file's wording.** `Arm::stop`'s doc and `parse_pursuit_stop`'s refusal message
  still call `ForwardHalfSpace` "the shipped rule". The `--pursuit-stop` *help* text, which
  lives in `main.rs`, has been corrected, and `apex-audit`'s CLI default moved to
  `reach-envelope` with it, so a bare audit runs the world as it ships.
- **No lifetime or death-cause comparison from the calibration rows.** A calibration row
  records neither; §3's apex table comes from `apex-audit` instead, at that instrument's own
  180,000-tick horizon rather than the screen's 360,000.
- **Nothing about viability, selection or recalibration.** Every apex member starves: 16 of 16
  in each audit arm, and every adult introduced in every calibration arm is dead by the horizon. The `baseline` arm-2 gate loss in §3 is reported, not acted on.

## 7. Verification

`cargo test -p cubarium-core` **548 passed / 0 failed / 4 ignored**;
`-p cubarium-search` **282 / 0 / 5**; `-p cubarium` **599 / 0 / 19**.
`cargo build --workspace --tests --release` clean. `graft build` rebuilt the graph.

The new tests, written from the brief's definitions and **run red before the flip** — four of
the five in `pursuit_predicate_adoption.rs` failed at `960b6c2`, and the fifth (the half-space
pin) is the one that must pass on both sides of the change:

| where | what it fixes |
| --- | --- |
| `cubarium-core/tests/pursuit_predicate_adoption.rs` (5) | an ordinary and a rebuilt world run the envelope untold; schema 17 refuses a schema-16 header over a valid payload, and every predecessor by number; the **resume regression** — saved at tick 5,500 and resumed, never told a rule, both sides on the envelope, identical hashes and identical paid attempts over 2,500 further ticks; a world told the half-space reproduces **six hashes pinned by `960b6c2`**; an untouched world reproduces `960b6c2`'s **envelope** hashes, and the two pinned sets agree before the first attempt the rules disagree about and part after it |
| `cubarium-search/tests/predicate_adoption_provenance.rs` (6) | the shipped default through `RunOptions` and every built layout world; a `StrikeRecord`, a `StagePlan` and a `PopulationPlan` with no rule each read `forward_half_space` and **not** today's default; no layout founds a hunter; the adoption moved no protocol hash |
| `cubarium-search/src/population.rs` (1) | an apex arm **runs** the rule it was handed: handing it the shipped rule is what an arm handed nothing already does, and handing it the older rule produces a different world — which is what a recorded-but-unapplied argument could not do |
| `cubarium/tests/run_neural_seed.rs` (2) | an `inertial` policy is refused by name and writes no world; a policy with no `motor` key seeds |
| `cubarium/tests/run_persistence.rs` (1) | `--fresh` writes schema 17; every snapshot relabelled 16 and nothing else changed is refused with `UnsupportedSchema(16)`, and the refused resume neither converts nor removes the world |

Three existing fixtures were re-pinned rather than weakened, each saying why in place:
`motor_inertial.rs`'s paired world now names `ForwardHalfSpace`, because its hashes were
printed by `2eb8a9f` when that was shipped and the pin is a statement about the *motor*
switch; `hunter_strike_record.rs`'s out-of-reach fixture names it too (a delivered burst lifts
the speed cap past the `speed_max = 0` it freezes with) and now additionally asserts that the
shipped rule would **not** hold that prey; `motor_foundation.rs`'s apex regression reads the
hunter's phase at **both ends** of the tick, because the Windup-to-Strike boundary tick spends
the strike budget and only became visible once the burst was delivered.

### The shared `target/` race, again, and a new face of it

The stale-artifact race E, K, N, P and T all documented recurred about five times here, and
once in a form worth recording: `SCHEMA_VERSION` is a `const`, so it is **inlined at compile
time**. A stale `cubarium` rlib linked against a fresh `cubarium-core` produces no link error
and no warning — it simply writes snapshots at the old number while the test crate beside it
reads the new one. Touching `crates/cubarium-core/src/lib.rs` alone does not clear that;
touching every crate's `lib.rs` and `main.rs` does. Every number in this note was taken from a
run made after such a rebuild, and the release binary was copied out of the shared target
directory before any row was run.

`cargo clippy -p cubarium-core --all-targets` still fails on the pre-existing `erasing_op`
deny in `crates/cubarium-core/src/neural/gru.rs:233`, untouched by this workstream.

## Artifacts

Under `runs/ecology-v1-predicate-adoption/` (git-ignored, 6.3 MiB against a 20 MiB cap), all
from search build `b3ac2ecV`:

- `holdout-half-space/{evals.jsonl,summary.json}` — the reproduction check
- `holdout-reach-envelope/{evals.jsonl,summary.json}` — the shipped default
- `audit-half-space.json`, `audit-reach-envelope.json` — the apex pair

Every command above reproduces them.
