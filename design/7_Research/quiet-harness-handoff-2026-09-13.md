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
art, no README or implementation-plan edit.

**Second pass.** The first build (`6d4ac7b`) was reviewed by root and by Astra, who between them
proved five real validation defects with read-only probes and six independent boundary
regressions. This revision fixes them rather than documenting them: the restart proof, the
reducer's limits, its horizon and cohort identity checks, its lifecycle reconciliation, and the
path measurement are all rebuilt. Everything root and Astra proved is now covered by a test that
fails without the fix. The section "What the review found, and what changed" below is the
itemised answer.

## Commit

`6d4ac7b` (first build) and `2fbd092` (this revision) — `crates/cubarium/examples/quiet_compare.rs`,
`crates/cubarium/examples/quiet_compare/bouts.rs`, `scripts/reduce-quiet-compare.mjs` and its
test, this report, and **one additive read-only core accessor**
(`World::moved_segments`, `crates/cubarium-core/src/world.rs`) authorized by the review. Nothing
else is touched. The accessor borrows the movement segments the render view already publishes; it
adds no field, changes no persisted shape, takes no step and draws no RNG, and a core test asserts
it is exactly what `render_view` publishes and that reading it leaves the state hash unchanged.

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

* every admission **and every refusal** matches a real paid `LifeEvent::Birth` at the same
  boundary with the same parent and child, and an admission's window is the candidate's;
* no admission for a newborn, a parent that is not alive, or a parent naming itself;
* every release and abort is matched back to **exactly one** earlier admission by parent, child
  and boundary — a close with nothing open, a different child, a different generation, or a tick
  that does not follow its boundary is a violation, not a number that happens to balance;
* the world's own pause set is reconciled both ways: a pause that appeared without an admission
  record, or vanished without a release or abort record, is a violation;
* every release completed exactly 40 held ticks; every abort completed fewer, **except** an abort
  whose parent died during its fortieth held interval, which legitimately completed forty and is
  counted separately as such;
* a recovery bout ends by the world's own release or abort record, never by "it was reclassified"
  — and what it went on doing next is kept as a second, separate fact;
* a held interval is Resting, took no intake, opened no new gestation, and drew exactly the
  ordinary four turn counters — so the override consumes no RNG of its own;
* the **exact** transported path, every tick, for every organism, from the world's own movement
  segments;
* a bounded restart proof comparing the **uninterrupted** arm against a decoded shadow (below).

### The restart proof

The first build compared two worlds decoded from the same bytes. Root proved that certifies two
restarts against each other, not an uninterrupted world against a restarted one: two decodes of a
snapshot that had lost a pause agree with each other perfectly, for the whole window. An
adversarial test now demonstrates exactly that, and the proof is rebuilt:

* the **primary** is the arm itself. It takes only the steps the experiment prescribes, draws only
  the RNG it would have drawn, and is never rebuilt from its own bytes to serve as its own oracle;
* a **shadow** is decoded once from a snapshot taken at `start_tick` and then stepped alongside
  the primary's own steps, receiving the identical prescribed Feed if the window crosses elapsed
  600, and compared every tick on the **whole persisted `WorldState`** and on the exact
  `LifeEvent` and `QuietEvent` streams;
* the trigger is recorded truthfully: `mid_pause` only when a pause really was open at
  `start_tick`, `fixed_boundary` otherwise. An arm that pauses only later still owes, and takes,
  the mid-pause proof;
* each proof records its start tick, finish tick, planned window, compared tick count, the pauses
  it carried and its scope. An incomplete proof, a proof left open at the close, or a missing
  mid-pause proof where a pause was open with a full window to spare fails the arm.

Measured limitation, stated because it is real: `QuietPause::underlying` is a **carry**, rewritten
from the ordinary hysteresis on every held decision. Corrupting it is caught where it is
persisted — the shadow's decode must equal the primary exactly before it may stand in for it —
but a *stepped* comparison is not a general detector of it, because the ordinary controller
re-derives the same answer within one tick for a parent outside the hysteresis band. The test says
so, and asserts the re-derivation rather than pretending the damage was caught.

Alongside: the unchanged strict inventories and independent flow/receipt audits from the existing
paired-experiment convention — one shared pre-intervention baseline per seed, material, water,
persisted compensated energy, independent windowed energy and immediate care-boundary energy, all
at the unchanged 1e-8 opening-inventory limits.

## Output

Per arm: `opening.json` (policy, care recipe, config SHA256, hashes before and after the choice,
shared baseline), `opening-organisms.jsonl` (**every organism that was there before anything was
chosen** — full generational ID, parent, form, genome digest, origin, birth tick, age, births,
mode, structure, reserve, energy, gestation, position), `summary.json`, `closing.cubw`, and four
streams:

* `bouts.jsonl` — every bout with full generational IDs, class, range, originating birth for
  recovery, how the **pause** ended, and separately what the organism went on doing next;
* `quiet-events.jsonl` — every begin/refuse/end/abort with reason and both identities;
* `life.jsonl` — **every** `Birth` and `Death`, with full IDs, parent, parent age and parity,
  genome digest, origin, mutated loci, form, opening ancestor and generational depth; a death
  keeps the form, birth tick and lineage it lived with, so a form that disappears entirely still
  has a record;
* `census.jsonl` — one line per 200 ticks: population by form, occupied cells, window births and
  deaths by cause, mode counts, escrows, stocks, surviving opening ancestry, open pauses, hashes.

There is also a read-only `--inspect <snapshot>` mode: it decodes one `.cubw` and prints its
header, identity, semantic inventories, derived audit limits, ledgers, care and quiet state as
JSON. Nothing is stepped, no `World` is constructed and nothing is written. The reducer uses the
run's **own frozen executable** in this mode, because a JSON reader parsing a header proves the
bytes are a snapshot and says nothing about what is inside it.

Nothing accumulates in memory. Bouts and life records are proportional to births, not to
organism-ticks, so the 72-hour horizon costs the same resident footprint as the ten-minute one.

**Measured from the 2400-tick smoke (48 arms in 21 s, 184 KB per arm of which 145 KB is the
fixed closing snapshot and opening census), extrapolated linearly:**

| horizon | output | wall clock |
| --- | ---: | ---: |
| ten-minute | ~11 MB | ~2 min |
| two-hour | ~47 MB | ~21 min |
| twenty-four-hour | ~480 MB | ~4.1 h |
| seventy-two-hour | ~1.4 GB | ~12.4 h |

Plus a 17 MB frozen executable copy per run directory. The extrapolation assumes a roughly stable
population; a growing or collapsing one moves both numbers, and the bout and life streams scale
with births rather than with time.

## Explicitly unsupported measurements

Stated in every arm summary rather than omitted. There is now **one**:

* **Exact funding and oxidation amounts.** The ordinary API offers no mutation-site evidence, so no
  post-step delta here is presented as either, and none is called causal.

Transported path length is no longer among them. The first build accumulated a same-face chart
delta and called the rest unsupported; root proved that shortcut is not even exact on same-face
movement at a reflective rim, and that a held parent's tiny drift *can* cross a seam when it
starts beside one. The harness now sums the world's **own** movement segments
(`World::moved_segments`, borrowed rather than cloned) every tick for every organism, so a seam
crossing is the pieces it really was and a rim's turn is inside them. Seam-crossing ticks are
counted as evidence that they are covered, not as an excuse for omitting them, and a test checks
the accumulated total against `render_view`'s own published segments over 600 ticks.

## What the review found, and what changed

Root's five proven findings and Astra's three failing regressions, each with the fix and the test
that fails without it.

| proved | fix | test |
| --- | --- | --- |
| The resume proof decoded the same bytes twice: it certified two restarts, not uninterrupted vs restarted | uninterrupted primary vs decoded shadow stepped along the primary's own timeline, identical prescribed care, whole persisted state and exact record streams, truthful `mid_pause`/`fixed_boundary` trigger, recorded range and scope, incomplete proof fails the arm | `a_damaged_shadow_is_caught_where_two_identical_decodes_agree` (and it demonstrates the two mirrors agreeing), `the restart proof must be complete, equal and genuinely mid-pause where one was open` |
| The reducer trusted self-reported limits, ignored the absolute care tick, the receipt outcome and unreceipted ledgers | limits re-derived from the opening snapshot's own inventories and the fixed 1e-8 rule; the receipt's seq, absolute tick, target, dose, applied quantities and the ledger it produced are all checked; a no-care arm's whole ledger must be zero field by field | `audit limits come from the opening inventory…`, `the care recipe is exactly one applied Standard Feed at the prescribed absolute tick`, plus root's own probe re-run (below) |
| `loadRun` demanded only `ticks >= 12000`, ignored `closing.cubw`, checked one census line, and enforced no identity | exact horizon name→ticks in the manifest and in every arm; the cohort compared against the directory it came from; the opening snapshot checksummed, header-parsed, CRC-checked and decoded; policy-only difference and one shared source-frozen baseline per seed; every census window in order with its flows summing to the run; every closing snapshot parsed, CRC-checked, checksummed and decoded for its semantics | `an opening is fingerprinted from the cohort source…`, `a closing snapshot is checked as bytes and then decoded…`, `every census window is present, in order…`, `a relabelled horizon is refused…` |
| Counts were reconciled, lifecycles were not | every close matched to exactly one admission by parent, child and boundary; the pause set reconciled both ways; bouts non-overlapping per organism with their promised next class; `life.jsonl` streams every birth and death by identity with lineage and form, and the opening identity census is written per arm | `every close belongs to exactly one admission…`, `the life stream is one population history…`, `bout files must agree with their own summary, line by line` |
| The same-face endpoint delta is not the transported path | the world's own movement segments, every tick, every organism | `the_transported_path_is_the_worlds_own_published_movement`, `moved_segments_is_exactly_what_the_render_view_publishes` |
| (Astra) A release into ordinary rest closed as `Reclassified(Satiated)`, losing the terminal | a recovery bout ends by the world's own record; what it became next is a separate field | `a recovery bout that loses its release or abort record is refused`, Astra's `astra_release_into_natural_rest_keeps_the_actual_release_reason` |
| (Astra) A parent dying on its fortieth held interval was rejected as a malformed abort | that interval is counted — the bout covers it and the abort's `completed_ticks` equals the bout's length — and is separated from an early abort-before-decision by reason and by its own counter | `a parent that dies on its fortieth held interval is counted, not smoothed away`, Astra's `astra_death_on_last_held_interval…` |
| (Astra) A missing `Begin` and an invented `Refuse` were both certified | the world's pause set is reconciled against the records both ways, and every refusal must match a real paid birth | Astra's `astra_missing_begin_record…` and `astra_unknown_refusal…` |

Root's own probe, unmodified, run as
`node design/7_Research/assets/root-quiet-gate-probe.mjs captures/quiet-smoke-validated-2026-09-13c`,
now reports `gap: false` on all five cases: the untouched summary is still accepted, and the
inflated limit, the unreceipted no-care ledger, the wrong absolute application tick and the
rejected receipt are each refused by name. Its own recorded reducer SHA256 moves from
`e3ea6233…` to `5cc9d876…`.

## Tests and results

| Command | Result |
| --- | --- |
| `cargo test -p cubarium --example quiet_compare` | **18 passed, 0 failed** |
| `cargo test -p cubarium --example astra_quiet_observer` | **14 passed, 0 failed** (all six independent regressions, previously three failing) |
| `cargo test -p cubarium-core` | **150 passed, 0 failed, 2 ignored** |
| `node scripts/reduce-quiet-compare.test.mjs` | **24 passed, 0 failed** |
| `cargo clippy -p cubarium --example quiet_compare -p cubarium-core` | no findings in any file touched here |

The example's tests cover the preregistered family and the named horizons; an arm changing the
policy and nothing else; both Off arms as ordinary continuations **including** the Standard Feed,
step for step against a plain world; a candidate arm admitting reconciled pauses and passing a
genuine mid-pause restart proof; a deliberately damaged shadow; the transported path against
`render_view`'s own segments; synthetic boundary, aborted, released and censored bouts; the
release and abort boundaries as ordinary decisions; an adversarial unbooked change failing the
audit at the unchanged limit in all three currencies; a tampered or incomplete cohort refused six
ways plus a flipped byte; the loader's per-opening check driven on real decoded openings (the
earlier version of that test asserted a tautology and never called the loader); and an exclusive
output path.

The reducer's tests drive every refusal from synthetic fixtures, parse and CRC-check a
hand-assembled snapshot, and then run all **48 real smoke arms** through every exported check
including the life stream, the census, the opening identity census and the decoded closing
snapshots.

### Smoke (technical only, 2400 ticks, certifies nothing)

`captures/quiet-smoke-validated-2026-09-13c`, from the release build, exit 0, all 48 arms
`technical_complete` and `audit_passed`, every gate true, `complete_experiment_measurement` false
because a smoke is not a prescribed horizon. Earlier runs of this build are kept beside it
(`captures/quiet-smoke-validated-2026-09-13`, `…-13b`) and the original pre-review smoke
(`captures/quiet-smoke-probe`) is untouched.

| arm | admissions | releases | aborts | refusals | recovery ticks | recovery bouts | newborn bouts | satiated bouts | transported px | seam ticks |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| off_nocare | 0 | 0 | 0 | 0 | 0 | 0 | 83 | 0 | 22006.91 | 478 |
| candidate_nocare | 26 | 26 | 0 | 57 | 1040 | 26 | 83 | 0 | 22023.02 | 470 |
| off_feed | 0 | 0 | 0 | 0 | 0 | 0 | 83 | 0 | 21915.83 | 479 |
| candidate_feed | 26 | 26 | 0 | 57 | 1040 | 26 | 83 | 0 | 21932.08 | 471 |

Reconciling: newborn bouts equal births exactly in every arm (83); recovery ticks are exactly
40 × releases; admissions plus refusals equal births exactly (26 + 57 = 83), so every real birth
produced exactly one offer; all 57 refusals were `unaffordable`; each arm streamed 137 life
records (83 births + 54 deaths) that reconcile to its own summary by identity; ten of twelve
candidate seeds proved a genuine mid-pause restart and the other two — which never paused in 2400
ticks — recorded the fallback as the fallback.

**The observation is inert, and this is the measurement that says so:** all 48 closing state
hashes are identical to the pre-review smoke's, byte for byte, across the new segment accounting,
the life stream, the opening census, the shadow proof and the inspect mode.

Two observations worth flagging for the review, neither of which is a result:

* **Natural satiated rest was zero in all 48 arms**, reproducing the frozen diagnostic's finding
  that active→Resting never occurs at these settings. The candidate is therefore the only source
  of non-newborn rest in this world.
* **Ten of twelve seeds produced at least one recovery bout in 2400 ticks**, and roughly a third of
  births were admitted (26 of 83). That is a technical observation from a smoke far below any
  prescribed horizon; it is **not** the proposal's behavioural screen, which needs the ten-minute
  no-care arms and is reported by the reducer, never asserted as success.

## Defects found and fixed during the first build

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
* **The whole-cohort reduction has not been run end to end**, because the only thing it will
  accept is a prescribed horizon and the ten-minute collection is not launched. Every one of its
  parts — each per-arm check, the snapshot inspection, the opening fingerprint, the life stream,
  the census, the closing snapshots — is exercised against the real 48 smoke arms, and the smoke
  is verified to be *refused* as a horizon. The glue that joins them is not.
* **Mutation-site attribution stays unsupported.** Funding and oxidation amounts are not derivable
  from the ordinary API, and no post-step delta here is presented as either.
* **The carried underlying mode is not detectable by a stepped restart comparison**, as measured
  above; it is checked where it is persisted.
* **Raw and compensated energy remain separate numbers.** `legacy_raw_energy` is reported as its
  own gate beside the corrected, windowed and care-boundary residuals; none of them is widened,
  merged or traded off against another, and every limit is still 1e-8 of the opening inventory.

## Independent review artifacts

* root: `design/7_Research/root-quiet-harness-gate-review-2026-09-13.md` and its probe under
  `assets/` — re-run above, all five cases now `gap: false`.
* Astra: `crates/cubarium/examples/astra_quiet_observer.rs` and
  `design/7_Research/astra-quiet-observer-review-2026-09-13.md` — six independent regressions,
  three of which failed against the first build and all of which pass now. Astra's file is
  unmodified.

## Next command, when review clears it

```text
CARGO_TARGET_DIR=captures/build-cache/quiet-harness \
  cargo build --release -p cubarium --example quiet_compare
captures/build-cache/quiet-harness/release/examples/quiet_compare \
  captures/hunter-openings-2026-09-13 captures/quiet-ten-minute-<hash> --horizon ten-minute
node scripts/reduce-quiet-compare.mjs captures/quiet-ten-minute-<hash>
```

The reducer refuses anything short of a prescribed horizon **named as what it is**, a 200-tick
cadence, twelve seeds, four arms, a frozen executable matching its manifest, and a reachable
cohort directory whose manifest is identical to the one the run recorded. It takes the cohort
directory as an optional second argument when the recorded path is no longer where it was. It
cannot be pointed at a smoke by mistake: `node scripts/reduce-quiet-compare.mjs
captures/quiet-smoke-validated-2026-09-13c` exits 1 with `smoke is not a prescribed horizon`.
