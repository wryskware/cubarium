---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Ordinary-quiet four-arm comparison harness: handoff

Implementation record for the comparison required by
[the ordinary quiet proposal](astra-ordinary-quiet-experiment-proposal-2026-09-13.md), built on
the committed `post_birth_pause_v1` core package (`0980eab`, handoff
[here](quiet-post-birth-pause-handoff-2026-09-13.md)). Evidence, not decisions.

**The study is not launched.** This is the frozen harness and its tests only. The 48-arm
ten-minute collection and every longer horizon wait on root/Astra review. No live deployment, no
core change, no art, no README or implementation-plan edit.

## Commit

`<this commit>` — `crates/cubarium/examples/quiet_compare.rs`,
`crates/cubarium/examples/quiet_compare/bouts.rs`, `scripts/reduce-quiet-compare.mjs` and its
test, plus this report. Nothing else is touched: the change footprint is four new files.

## The fixed contract, as built

Twelve prescribed openings from `captures/hunter-openings-2026-09-13`, verified before anything is
constructed from them — the file's own bytes against the manifest checksum, then schema 9, tick
144000, seed, census and ecology hash, no care, no hunter, and no quiet extension already present.
The originals are read-only inputs.

| | no care | one Standard Feed |
| --- | --- | --- |
| **Off** (reference) | `off_nocare` | `off_feed` |
| **candidate** `post_birth_pause_v1` | `candidate_nocare` | `candidate_feed` |

The care recipe is exactly one Standard Feed (dose 1000) at elapsed tick 600, Front (32, 48), and
nothing after it — no later rescue, no cleanup, no rain. Ambient support is identical in all four
arms. No hunters. The Off arm never writes the quiet field; the candidate writes the policy once,
to a copied state, before `World::from_state`, and an arm refuses at construction if anything
other than that field differs from the opening.

Horizons are named, not relabelled: `ten-minute` 12000, `two-hour` 144000, `twenty-four-hour`
1728000, `seventy-two-hour` 5184000, plus a `smoke` that can never certify a measurement. The
census cadence is 200 ticks and the completion flag additionally requires it, so a run with a
different cadence cannot claim to be the prescribed one.

## Truthful rest classification

This is the part that had to be built rather than reused. The frozen ten-minute diagnostic counted
rest by reading `Mode::Resting` and assuming every decision came from the ordinary controller —
which is exactly the assumption the candidate breaks. `quiet_compare/bouts.rs` classifies each
completed interval from actual core records plus the pause set captured **before** the decision
that produced it:

| class | decided by |
| --- | --- |
| `newborn_initial` | the organism's own `born_tick` equals this completed tick |
| `post_birth_recovery` | a pause existed before the decision, `holds(T)` was true, and no `Abort` was published for it at `T` |
| `satiated` | Resting and neither of the above — the ordinary controller chose it |

Both boundaries a naive reading gets wrong are handled explicitly. At `T == end_tick` the entry
still exists but holds no decision: that is the ordinary **release**, and the interval it produces
is not recovery. At an early **abort** the entry existed and held, but the world abandoned the
pause before deciding, so that interval is not recovery either — the abort record at `T` says so.
Both are covered by their own tests.

Every organism-tick is classified exactly once, and the reducer checks that the classified ones
are exactly the resting mode-ticks. A total that no bout line supports is refused.

## What the harness proves per arm, tickwise

Recorded as counts with the first sixteen violations kept verbatim; **any** violation makes the
arm not a completed measurement, exactly like a conservation drift.

* every admission matches a real paid `LifeEvent::Birth` at the same boundary with the same parent
  and child, and its window is the candidate's;
* no admission for a newborn, a parent that is not alive, or a parent naming itself;
* every release completed exactly 40 held ticks; every abort completed fewer;
* a held interval is Resting, took no intake, opened no new gestation, and drew exactly the
  ordinary four turn counters — so the override consumes no RNG of its own;
* actual motion is accumulated rather than assumed;
* a bounded once-per-arm snapshot→decode→rebuild→lockstep proof, taken at the first admitted pause
  where there is one so it is a genuine mid-pause restart, comparing state hashes and both record
  streams for 60 ticks.

Alongside: the unchanged strict inventories and independent flow/receipt audits from the existing
paired-experiment convention — one shared pre-intervention baseline per seed, material, water,
persisted compensated energy, independent windowed energy and immediate care-boundary energy, all
at the unchanged 1e-8 opening-inventory limits.

## Output

Per arm: `opening.json` (policy, care recipe, config SHA256, hashes before and after the choice,
shared baseline), `summary.json`, `closing.cubw`, and three streams — `bouts.jsonl` (every bout
with full generational IDs, class, range, originating birth for recovery, and how it ended),
`quiet-events.jsonl` (every begin/refuse/end/abort with reason and identity), `census.jsonl` (one
line per 200 ticks: population by form and face, occupied cells, window births and deaths by
cause, mode counts, escrows, stocks, surviving opening ancestry, open pauses, hashes).

Nothing accumulates in memory. Bouts are proportional to births, not to organism-ticks, so the
72-hour horizon costs the same resident footprint as the ten-minute one.

**Measured from the smoke, extrapolated linearly (48 arms):**

| horizon | output | wall clock |
| --- | ---: | ---: |
| ten-minute | ~8 MB | ~2 min |
| two-hour | ~36 MB | ~21 min |
| twenty-four-hour | ~365 MB | ~4.2 h |
| seventy-two-hour | ~1.1 GB | ~12.6 h |

Plus a 17 MB frozen executable copy per run directory. The extrapolation assumes a roughly stable
population; a growing or collapsing one moves both numbers, and the bout stream scales with
births rather than with time.

## Explicitly unsupported measurements

Stated in every arm summary rather than omitted:

* **Total transported path length at full tick resolution.** The world's own per-tick segments are
  reachable only through `World::render_view`, which clones four whole fields per call — fine at a
  census cadence, not 5.18 million times per arm. So the harness accumulates same-face chart
  displacement exactly every tick, counts the ticks it omits because the organism changed face,
  and separately records the world's exact transported length on the census cadence as a **rate**.
  A reader gets an exact partial total, an exact count of what it omits, and an exact sampled
  rate — never a fabricated whole.
* **Exact funding and oxidation amounts.** The ordinary API offers no mutation-site evidence, so no
  post-step delta here is presented as either, and none is called causal.

## Tests and results

| Command | Result |
| --- | --- |
| `cargo test -p cubarium --example quiet_compare` | **16 passed, 0 failed** |
| `node scripts/reduce-quiet-compare.test.mjs` | **10 passed, 0 failed** |
| `cargo clippy -p cubarium --all-targets` | no findings in either new file |

The example's tests cover the preregistered family and the named horizons; an arm changing the
policy and nothing else; both Off arms as ordinary continuations **including** the Standard Feed,
step for step against a plain world; a candidate arm admitting reconciled pauses and passing its
mid-pause restart proof; synthetic boundary, aborted, released and censored bouts; the release and
abort boundaries as ordinary decisions; an adversarial unbooked change failing the audit at the
unchanged limit in all three currencies; a tampered or incomplete cohort refused six ways plus a
flipped byte; and an exclusive output path.

The reducer's tests drive every refusal from synthetic fixtures and then run all **48 real smoke
arms** through the same exported checks.

### Smoke (technical only, 2400 ticks, certifies nothing)

`captures/quiet-smoke-probe`, from the release build, exit 0, all 48 arms `technical_complete` and
`audit_passed`, every gate true.

| arm | admissions | releases | aborts | refusals | recovery ticks | recovery bouts | newborn bouts | satiated bouts |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| off_nocare | 0 | 0 | 0 | 0 | 0 | 0 | 83 | 0 |
| candidate_nocare | 26 | 26 | 0 | 57 | 1040 | 26 | 83 | 0 |
| off_feed | 0 | 0 | 0 | 0 | 0 | 0 | 83 | 0 |
| candidate_feed | 26 | 26 | 0 | 57 | 1040 | 26 | 83 | 0 |

Reconciling: newborn bouts equal births exactly in every arm (83); recovery ticks are exactly
40 × releases; admissions plus refusals equal births exactly (26 + 57 = 83), so every real birth
produced exactly one offer; all 57 refusals were `unaffordable`; the fed arms' closing hashes
differ from their no-care counterparts, and the candidate arms differ from Off.

Two observations worth flagging for the review, neither of which is a result:

* **Natural satiated rest was zero in all 48 arms**, reproducing the frozen diagnostic's finding
  that active→Resting never occurs at these settings. The candidate is therefore the only source
  of non-newborn rest in this world.
* **Ten of twelve seeds produced at least one recovery bout in 2400 ticks**, and roughly a third of
  births were admitted (26 of 83). That is a technical observation from a smoke far below any
  prescribed horizon; it is **not** the proposal's behavioural screen, which needs the ten-minute
  no-care arms and is reported by the reducer, never asserted as success.

## Defect found and fixed during the build

The first smoke reported ~8700 births per arm in 2400 ticks. `TickCounters` returned by
`World::step` accumulate since the last **telemetry reset** rather than describing one tick, so
adding them every tick multiplied every event by the census cadence. Births and deaths are now
counted from the world's own per-tick `LifeEvent` records, with their full IDs and causes, which
also makes them reconcile against the bout and ancestry data — newborn bouts now equal births
exactly. Predation is carried as a fourth death slot so a nonzero value would be visible rather
than folded into another.

The reducer also caught a real inconsistency in the harness's own output: core's derived encoding
spells a reason `"Unaffordable"` while every summary spells it `"unaffordable"`. Two spellings for
one thing in one artifact set is a trap for whoever reads it later, so the record stream is now
written in the harness's own flat shape with a `kind` field and the same labels the summaries use.

## Limitations

* **The study has not run.** Every number above is from a 2400-tick technical smoke. Nothing here
  is evidence about quiet opportunity, viability, recruitment or balance.
* **Forty ticks remains unvalidated**, and Off remains the autonomous default.
* **A technical pass is not a behavioural one.** The proposal's screen — a non-newborn recovery
  bout of at least one second in at least nine of twelve no-care seeds — is computed and reported
  by the reducer and deliberately never gates anything. A candidate whose entries are all one tick
  has not met the objective whatever that number says.
* **The reducer reaches no verdict.** It reports per-seed paired deltas and every paired loss —
  new extinctions, forms or opening ancestry lost where the matched reference kept them, fewer
  births — and states plainly that viability is decided from the no-care arms by inspecting each
  pair, not from a pooled mean.
* **Twelve seeds cannot prove universal noninferiority**, at any horizon.
* **The runtime and storage figures are linear extrapolations** from a short smoke on a
  roughly-stable population.

## Next command, when review clears it

```text
CARGO_TARGET_DIR=captures/build-cache/quiet-harness \
  cargo build --release -p cubarium --example quiet_compare
captures/build-cache/quiet-harness/release/examples/quiet_compare \
  captures/hunter-openings-2026-09-13 captures/quiet-ten-minute-<hash> --horizon ten-minute
node scripts/reduce-quiet-compare.mjs captures/quiet-ten-minute-<hash>
```

The reducer refuses anything short of a prescribed horizon, a 200-tick cadence, twelve seeds, four
arms and a frozen executable matching its manifest, so it cannot be pointed at a smoke by mistake.
