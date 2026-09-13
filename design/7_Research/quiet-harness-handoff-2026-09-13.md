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

**The ten-minute screen has now been collected and reduced; nothing longer has.** Root ran the
first 48-arm ten-minute collection from a frozen `42faaa2`, and its reduction **failed** on
cohort provenance. That defect is fixed here, and a corrected collection was run and reduced end
to end (below). Every horizon beyond ten minutes still waits on root/Astra review, and no
completion flag in any of it is evidence that the candidate helps anything. No live deployment,
no art, no README or implementation-plan edit.

**Second and third passes.** The first build (`6d4ac7b`) was reviewed by root and by Astra, who
between them proved five validation defects with read-only probes and six independent boundary
regressions; `2fbd092` fixed those. Astra then reviewed that correction (`5479e11`) and proved
four narrower ones — a lost one-tick bout, a false refusal at the completed `B+40` boundary, an
energy contract the reducer had tightened past the runner's own, and a cross-file link that
compared a parent and a boundary but dropped the child. This revision fixes those four.
Everything root and Astra proved is covered by a test that fails without the fix. The sections
below are the itemised answer.

## Commit

`6d4ac7b` (first build), `2fbd092` (first correction), `fe114f9` (second correction) and
`9fc29eb` (cohort provenance) —
`crates/cubarium/examples/quiet_compare.rs`,
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

## What the first review found, and what changed

Root's five proven findings and Astra's six independent regressions — three of which were failing
against `6d4ac7b` — each with the fix and the test that fails without it.

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
`e3ea6233…` to `5cc9d876…` and, after the second correction below, to `67cbb8c9…`.

## What the second review found, and what changed

Astra's read-only review of `2fbd092`/`d3f9782` confirmed those closures and proved four narrower
defects (`5479e11`). All four are fixed here.

| proved | fix | test |
| --- | --- | --- |
| A parent that dies during its **first** held interval loses the whole bout. The death branch iterated only already-open bouts, and at the admitting boundary the parent was still active, so no recovery bout existed to extend. The core's real `Abort(ParentGone, completed_ticks = 1)` was reconciled against nothing. | held deaths are now found from the pause set captured before the decision and the world's own death record, independently of whether a bout was open: the interval opens the bout it deserves, or extends the one already running, or displaces another class's bout that the pause had taken over | Astra's `astra_death_on_first_held_interval_retains_the_actual_one_tick_bout`; mine, `a_death_in_the_first_held_interval_is_a_one_tick_bout_matching_its_abort`, asserts the property the reducer depends on — the bout's length equals the abort's `completed_ticks` |
| `verifyClosingSnapshot` required `tick < end_tick`, so a genuine completed `B+40` state — which still carries the entry until the following ordinary decision — was falsely refused | the range is the core's own: `start_tick <= tick <= end_tick`, equality at the completed boundary and nothing past it | Astra's `a completed B+40 snapshot may retain its pause until the following decision`; mine adds the reciprocal, that one tick past the decision and one whole window early are both still refused |
| The reducer demanded `gates.legacy_raw_energy` and gated raw energy drift, which is stricter than the runner's own `shared/audit.rs::audit_passes` — that gates raw material and water plus the persisted-corrected, independently windowed and immediate care-boundary energy, and keeps raw legacy energy as a diagnostic. An arm could truthfully pass the runner and be refused here for a naive counter's rounding. | the reducer now uses exactly the runner's contract at exactly the same unchanged 1e-8 limits. The raw drift and its honest flag are retained, checked for being real numbers, returned by `verifyArm` and reported per arm in the paired output — preserved, never relabelled as passing | Astra's `legacy raw energy remains diagnostic when all fixed corrected energy gates pass`; mine, `the energy contract here is the runner's own…`, also asserts that none of the three compensated residuals and neither raw material nor raw water was relaxed with it |
| The cross-file paid-birth link dropped the child: the offer key carried `parent@tick>child` but the predicate compared only `parent@tick`. Astra substituted the child of a real Begin/End pair from `11/5` to `11/100005` at `47/5@144228` in the actual smoke, and every component reducer and the crosswalk accepted it | the crosswalk is now an exported `crossCheck` over whole identities: every offer must name a child that parent really paid for at that boundary (a parent with two insertions on one tick keeps both), every recovery bout's origin child must be its admission's, and — reciprocally — every close with completed intervals must have a bout of exactly that length, every non-censored bout must have the record that closed it, and a censored bout must have none | `a substituted child generation is refused although every stream stays self-consistent` replays Astra's exact mutation on the real seed-1 records, and separately relabels the bout side; `the crosswalk compares whole identities, both ways round` drives every missing, extra and mismatched case through the predicate |

Astra's review also corrected a wording slip here: all **six** of its first-package regressions
pass, three of which were failing before `2fbd092`. That is what the table above now says.

## What the first real reduction found, and what changed

Root froze `42faaa2`, built it, ran all 48 arms of the ten-minute screen into
`captures/quiet-ten-minute-42faaa2` — every arm complete and audited — and the **reduction
refused it**. It was right to.

`load_cohort` parsed the cohort manifest with `serde_json`, and `main` wrote that parsed `Value`
back out as the run's record of its cohort. `serde_json` 1.0.151 gates its exact decimal path
behind the opt-in `float_roundtrip` feature, which this workspace does not enable, so its default
fast path can land **one ULP** from the correctly rounded value. Three preparation telemetry
decimals came out changed:

| in the cohort on disk | in the run's copy of it | apart |
| --- | --- | ---: |
| `212.54356731997558` | `212.5435673199756` | 1 ULP |
| `0.9785584621020161` | `0.978558462102016` | 1 ULP |
| `60.830572942452996` | `60.83057294245299` | 1 ULP |

These are **different doubles**, not different spellings of one — `Number(a) !== Number(b)` for
each pair, and each source text is already the shortest round-tripping form. So the run's own
record disagreed with its source, and an exact checker had to refuse it. The cause is proven from
the installed crate rather than assumed: `serde_json_can_move_a_cohort_decimal_by_one_ulp` parses
each literal through `serde_json` and through Rust's correctly rounded parser and measures the
drift. Nothing in the science moved: worlds are built from the `.cubw` snapshots, whose bytes are
checksummed against the manifest, and every value the screen uses — seed, population, SHA256,
ecology hash, schema, tick — is an integer or a string.

The fix is to stop re-encoding:

* the run copies the cohort manifest **byte for byte** to `cohort-manifest.json` and records its
  `sha256` and length in `cohort_manifest`;
* `cohort_summary` reproduces only what can be reproduced exactly — the integers and strings the
  screen uses — and each opening row points at the copy for the float-bearing preparation
  telemetry rather than writing it again from a parse;
* the reducer checks the copy against its recorded checksum and length, against the source
  directory **byte for byte**, then parses that one set of bytes and checks `cohort_summary`
  field by field; a decimal in it is refused outright.

No tolerance was introduced anywhere and no metadata was dropped — the preparation telemetry is
present and unaltered in the copied manifest, which is more than the re-encoded record held.

**Root's 48-arm run stays exactly as it is, and stays failed.** Its embedded cohort copy holds
numbers that are not its source's, so no exact route certifies it. A labelled scratch
reconstruction — `captures/scratch-quiet-ten-minute-42faaa2-diagnostic`, those same artifacts with
a byte-faithful cohort record added — passes every other check in the reduction. That says the
provenance record was the only defect in it. It is a diagnostic, not a certification: it contains
a file the run did not write.

## Tests and results

| Command | Result |
| --- | --- |
| `cargo test -p cubarium --example quiet_compare` | **20 passed, 0 failed** |
| `cargo test -p cubarium --example astra_quiet_observer` | **16 passed, 0 failed** — all **seven** independent observer fixtures, including the new first-held-interval death |
| `cargo test -p cubarium-core` (whole crate) | **349 passed, 0 failed** |
| `cargo test -p cubarium-core --test astra_quiet_policy` | **9 passed, 0 failed** |
| `node scripts/reduce-quiet-compare.test.mjs` | **31 passed, 0 failed** — including the whole reduction end to end and thirteen tampered copies of it |
| `node scripts/astra-quiet-correction-review.test.mjs` | **4 passed, 0 failed** (all were red when written) |
| `cargo clippy -p cubarium --example quiet_compare` | no findings in any file touched here |

The JS suite now **requires** two artifacts and says so instead of skipping: the smoke it reads
(`captures/quiet-smoke-provenance-2026-09-13`) and the completed ten-minute screen
(`captures/quiet-ten-minute-provenance-2026-09-13`). Silently skipping the end-to-end path is how
the first real reduction came to fail on glue nothing had ever run; a missing artifact now fails
with the command that produces it.

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

### The corrected ten-minute screen, collected and reduced

One collection, from a pinned build, into an exclusive new directory. Nothing longer was run.

| | |
| --- | --- |
| frozen source | `captures/experimental-source/quiet-screen-provenance-2026-09-13`, `git archive 9fc29eb` |
| build | `0.1.0+9fc29eb`, release, `CARGO_TARGET_DIR=captures/build-cache/quiet-screen-provenance` |
| executable SHA256 | `876b9ade000f33cc03761e0c5547bc648b237ca91288a4b62ccfaa681e4540db` |
| cohort | `captures/hunter-openings-2026-09-13`, manifest SHA256 `f6f8ff36c23d75d1bd5fc1c4d3ab245dc969a74a37900c31d2a4e215f2a5bf8d`, copied byte for byte into the run |
| run | `captures/quiet-ten-minute-provenance-2026-09-13`, 48 arms, 12000 ticks each, 1 min 43 s, exit 0 |
| run gates | `technical_complete` true, `audit_passed` true, `complete_experiment_measurement` true |
| reduction | `captures/quiet-ten-minute-provenance-2026-09-13/reduction.json`, exit 0, `artifact_checks_passed` true |

**This is the first end-to-end reduction of a prescribed horizon that completed.** It means the
artifacts are internally consistent and faithful to their inputs. It is not a biological result,
and `complete_experiment_measurement` is a statement about audited numerical coverage, not about
the candidate being good for anything.

What the reduction *reports*, for root and Astra to interpret — not interpreted here:

| seed | births | admissions | refusals | recovery bouts | bouts ≥ 1 s | longest | held-interval deaths |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 33 | 11 | 22 | 11 | 11 | 40 | 0 |
| 2 | 39 | 16 | 23 | 16 | 16 | 40 | 0 |
| 3 | 33 | 6 | 27 | 6 | 6 | 40 | 0 |
| 4 | 37 | 10 | 27 | 10 | 10 | 40 | 0 |
| 5 | 27 | 4 | 23 | 4 | 4 | 40 | 0 |
| 6 | 38 | 12 | 26 | 12 | 12 | 40 | 0 |
| 7 | 32 | 4 | 28 | 4 | 4 | 40 | 0 |
| 8 | 21 | 3 | 18 | 3 | 3 | 40 | 0 |
| 9 | 42 | 14 | 28 | 14 | 14 | 40 | 0 |
| 10 | 39 | 8 | 31 | 8 | 8 | 40 | 0 |
| 11 | 21 | 4 | 17 | 4 | 4 | 40 | 0 |
| 12 | 41 | 14 | 27 | 14 | 14 | 40 | 0 |

The proposal's behavioural screen reads 12 of 12 no-care seeds, and the reducer reports it
without gating on it, exactly as before. Recovery is between 1.1e-4 and 5.6e-4 of organism time
in the candidate no-care arms.

**Paired losses are present and are the thing to look at.** Counting seeds where the candidate
lost something its matched reference kept: in the no-care pairs, 6 seeds lost opening ancestry,
8 had fewer births and 5 had more opening-organism deaths; in the fed pairs, 5, 6 and 4. No seed
went extinct and no form was lost in either. Whether that is acceptable is a biological judgement
for root and Astra from the per-seed pairs in `reduction.json`; this record does not make it, and
a passing artifact check is not an argument in either direction.

### Smoke (technical only, 2400 ticks, certifies nothing)

`captures/quiet-smoke-validated-2026-09-13c`, from the release build, exit 0, all 48 arms
`technical_complete` and `audit_passed`, every gate true, `complete_experiment_measurement` false
because a smoke is not a prescribed horizon. Earlier runs of this build are kept beside it
(`captures/quiet-smoke-validated-2026-09-13`, `…-13b`) and the original pre-review smoke
(`captures/quiet-smoke-probe`) is untouched. Astra's own validator fixtures read `…-13c`, so it
stays exactly where it is; the suite's own smoke-backed checks moved to
`captures/quiet-smoke-provenance-2026-09-13`, which carries the byte-faithful cohort record.

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

The second correction touches the observer's death branch, so a confirmation smoke was run into
a new directory — `captures/quiet-smoke-validated-2026-09-13d`, from a different build id
(`0.1.0+1d648bf` → `0.1.0+61921e3`). Against `…-13c` it is identical in all 48 closing state and
ecology hashes, all 48 rest observers, and all five output streams of all 48 arms byte for byte,
with `held_intervals_ended_by_death` zero across the cohort. So the new branch changes nothing
where no parent dies mid-pause, and `…-13c` remains valid reviewed evidence — which is also why
the reducer's tests and Astra's fixture still read `…-13c`. All 48 arms of `…-13d` were run
through every exported check including the crosswalk and the decoded closing snapshots, and
`node scripts/reduce-quiet-compare.mjs captures/quiet-smoke-validated-2026-09-13d` still exits 1
with `smoke is not a prescribed horizon`.

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
* **The reduction has now run end to end, once, on one horizon.** That closes the gap this record
  used to name — and the first attempt at it is exactly what found the cohort provenance defect.
  Nothing longer than ten minutes has been collected or reduced.
* **A completed reduction is an artifact statement.** It says the records are internally
  consistent, faithful to the cohort they came from, and cover the horizon they claim. It says
  nothing about quiet opportunity, viability or benefit, and no flag in it should be read as
  approval to change the Off default.
* **Mutation-site attribution stays unsupported.** Funding and oxidation amounts are not derivable
  from the ordinary API, and no post-step delta here is presented as either.
* **The carried underlying mode is not detectable by a stepped restart comparison**, as measured
  above; it is checked where it is persisted.
* **Raw and compensated energy remain separate numbers, and only the compensated ones gate.**
  That is the runner's own contract (`shared/audit.rs::audit_passes`): raw material and raw water,
  the persisted compensated energy, the independently windowed energy and the immediate
  care-boundary energy. The raw legacy energy drift is a **diagnostic** — retained, reported per
  arm, and never relabelled as passing. Nothing is widened, merged or traded off, and every limit
  is still 1e-8 of the opening inventory.
* **The observer's transported path and organism-tick totals cover post-step living organisms.**
  A dead organism's final movement segment is not among them; the held interval it died in is
  counted as a bout and as `held_intervals_ended_by_death`, which is deliberately kept out of the
  population-time denominator. No claim is made about every pre-step creature's terminal motion,
  and terminal intake or RNG checks for a body that is already gone remain unavailable.
* **Recovery entries are not separated into "interrupted activity" and "already resting".** The
  classification says an interval was held; it does not say the parent would otherwise have been
  moving. Do not read admissions as interruptions of activity.
* **What is validated here is recorded evidence, not replayed biology.** The care checks verify a
  receipt and the ledger it produced; the snapshot checks verify a decode of bytes this run
  wrote. Neither re-runs the world, and no test passing here is evidence of ecological benefit,
  adequate quiet opportunity or a reason to change the Off default.
* **Cohort provenance is byte equality, and it needs the cohort directory.** A run whose cohort
  directory has moved cannot be reduced until it is pointed at again (second argument). Runs
  written before `9fc29eb` have no byte-faithful cohort record at all and are refused by name —
  including root's `captures/quiet-ten-minute-42faaa2`, which stays as it is.

## Independent review artifacts

* root: `design/7_Research/root-quiet-harness-gate-review-2026-09-13.md` and its probe under
  `assets/` — re-run above, all five cases now `gap: false`.
* Astra: `crates/cubarium/examples/astra_quiet_observer.rs`,
  `scripts/astra-quiet-correction-review.test.mjs` and
  `design/7_Research/astra-quiet-observer-review-2026-09-13.md` — seven independent observer
  fixtures and two validator fixtures. Six observer fixtures came with the first review (three of
  them failing against `6d4ac7b`); the first-held-interval death and both validator fixtures came
  with the second (`5479e11`) and were failing against `2fbd092`. All nine pass now, and Astra's
  files are unmodified.

## Running a collection

This is what produced the ten-minute screen above, and what a longer horizon would use — from a
frozen source so the binary is tied to a reviewable tree:

```text
git archive <commit> | tar -x -C captures/experimental-source/<name>
cd captures/experimental-source/<name>
CARGO_TARGET_DIR=<repo>/captures/build-cache/<name> \
  cargo build --release -p cubarium --example quiet_compare
cd <repo>
captures/build-cache/<name>/release/examples/quiet_compare \
  captures/hunter-openings-2026-09-13 captures/quiet-<horizon>-<name> --horizon ten-minute
node scripts/reduce-quiet-compare.mjs captures/quiet-<horizon>-<name>
```

The reducer refuses anything short of a prescribed horizon **named as what it is**, a 200-tick
cadence, twelve seeds, four arms, a frozen executable matching its manifest, a byte-faithful copy
of the cohort manifest matching its recorded checksum, and a reachable cohort directory whose
manifest is that copy byte for byte. It takes the cohort directory as an optional second argument
when the recorded path is no longer where it was. It cannot be pointed at a smoke by mistake:
`node scripts/reduce-quiet-compare.mjs captures/quiet-smoke-provenance-2026-09-13` exits 1 with
`smoke is not a prescribed horizon`.
