---
status: open
date: 2026-10-12
owner: Opus (high), worktree `.claude/worktrees/contracts`, branch `contracts`
---

# CONTRACTS: the shared interfaces of behaviour-AI §11, in code

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first. It covers windows, the
5-minute limit, labd, builds, tests, knobs, saves, commits and the return format. Use graft
(`graft ask … --source`, `graft callers`) before grepping.

## Why

This is package 2 of `design/behaviour-ai-2026-10-11.md`. Read §0 (Wrysk's calls), §2
(signals, the intent layer, actions), §3.3–3.5 (kick, rest, bond), §10.1 (the two-layer
brain) and all of §11 (contracts, from Astra's review in
`design/7_Research/astra-behaviour-ai-review-2026-10-11.md`).

Packages 3–7 (calls and rest, loftstrider defence, Veilhound tactics, young roaming, the
two-layer brain) all share the controller write path, the action interface, observation
encoding, labels and saves (`step.rs:1783-1789`, `2171-2194`). This package lands that common
contract once, before they split (Astra #13).

**It ships records and interfaces, not behaviour.** A world with today's heuristics behaves
as today, with the new records filled in.

## Do

Commit after each numbered step.

1. **Intent vocabularies and records** (§2.2, §11.3, §10.2).
   - A per-lineage menu as data:
     - provisional named intents for the seedporter, loftstrider, Veilhound and lanternjaw,
       from §2.2, §3.1, §4 and §10.2's label sources;
     - two spare slots, kept masked;
     - plain lineages get a one-item menu.
   - On each animal, recorded apart:
     - the held intent;
     - its subject;
     - hold age;
     - the executed actions;
     - an override reason (DROP, wade-out, hit, and so on).
   - `Steer` stays the executed steering source.
   - The heuristics fill the held intent from their existing modes and `Steer` wherever the
     mapping is clear. This is what the two-layer brain clones (§10.2), so record the
     teacher's proposal before its own hold separately from what it held (§11.3, "What the
     top imitates").
   - Per-lineage knobs for the brain's arbitration: `hold_s`, `switch_margin` on
     probabilities, and the fixed temperature.
2. **Subjects and slots** (§11.1).
   - Typed subjects: none, a body (sensed or remembered), or a place (sensed or remembered,
     with its kind: rest spot, excursion goal, guard point, cut-off point).
   - A body subject is held by id and re-marked to its current slot at each sample.
   - Expiry after `subject_lost_s`.
   - Intent–subject validity masks.
   - Bounded kin, threat and prey slots carrying §11.1's fields. A recognised mother or calf
     keeps a reserved kin slot while sensed.
   - The menu lineages get a new manifest/observation version that encodes these. Plain
     manifests (frondgrazer, littershredder and the other plain prey) stay byte-identical
     (§11.6). Old menu-lineage policy files are refused with a clear error; no migration.
3. **The brain boundary** (shared with the TWO-LAYER package, below).
   - Extend the `Controller` seam (`crates/cubarium-voxel-fauna/src/controller.rs`) so a
     brain receives the intent mask and subject candidates, and returns intent, subject and
     the new actions.
   - Expose the brain's recurrent state for saving.
   - A plain lineage's call reduces exactly to today's.
4. **The action interface** (§11.4, §3.3, §3.4).
   - **Kick.** One state machine with one budget, shared by aimed kicks and kick-backs:
     windup, contact, recovery. Geometry follows the bites' convention, and the target is
     tracked during windup within the turn rate. Whether the body may step during windup is
     a knob.
     - Its damage and reach are knobs, sized from §3.3 and topic 6.
     - **It is enabled for no lineage by default.** LS-DEFEND turns it on for the loftstrider
       and routes the reactive counter through it.
   - **Rest request.** A brain or heuristic asks for rest through the action interface, and
     the body checks eligibility. Today's rhythm stays the backstop.
     - The heuristics route their existing rest decisions through the request, unchanged.
     - Light and deep physiology is CALLS-REST's.
   - **Call emission.** A call-kind action (none, or a lineage's kinds) enters the call
     substrate of step 5. No lineage emits a new call yet.
5. **Calls and hearing** (§11.2).
   - Call records: kind, bearing, proximity, age, and the caller's relation if the hearer
     recognises the voice (a per-lineage knob). Never an exact subject id.
   - Collected by their own range, not through the nearest-animal candidate cap
     (`perception.rs:601-628`).
   - A few records per hearer, with deduplication and a refractory time.
   - Occlusion by a terrain ray.
   - Asleep hearing above a loudness threshold, with the same occlusion. Record what a
     sleeper hears; waking on it is CALLS-REST's. Note how the current wake path
     (`rhythm.rs:371-408`) differs.
   - The frondgrazer's existing alarm stays as it is (CALLS-REST ports it).
6. **Saves** (§8 "Saving", §11.3).
   - Saved:
     - the held intent, subject, hold age and the brain's recurrent state;
     - the kick state;
     - the call records, if that's simpler than not saving them (they're harmless to lose).
   - Bump the fauna schema; take the next free number at commit time.
7. **Founder kinship** (§11.6). One family's founders are recorded as sisters, and one
   pack's as littermates: a shared founding mother id that names no living animal.
   - Everything that reads `Animal::mother` (nursing, the mother pull, the census) must
     treat that id as an absent mother.
   - Siblings are only those sharing a real recorded mother id.

## The interface shared with TWO-LAYER

A parallel package (`design/handoffs/two-layer-core-2026-10-12.md`) builds the two-layer
network in `crates/cubarium-search/` against this shape. Make yours meet it, or tell the
parent where it can't.

- **Per decision sample, the fauna side gives the brain:**
  - the observation vector, in manifest order;
  - the menu size and a validity mask over intents;
  - K subject-candidate rows, each:
    - a fixed width F per lineage;
    - a type tag (body or place);
    - a validity mask per intent;
  - an early-end flag with its reason: subject lost or dead, intent masked, or a physical
    emergency.
- **The brain answers with:**
  - today's continuous actions;
  - the discrete actions where the lineage has them: the kick trigger, the call kind
    (including none) and the rest request;
  - its held intent index and subject candidate index (or none), after its own hold and
    margin;
  - its proposal probabilities, for logging.
- **The fauna side** maps the candidate index back to a body id or place and records it.
- **Dimensions** come from the manifest at runtime.
- **A plain lineage:** menu size 1, K = 0, no discrete actions.

## Boundaries

- **Not this package:**
  - species calls and reactions to them;
  - light and deep rest;
  - the loftstrider's bond and defence;
  - the Veilhound's tactics, strike identity, blocking, and the `window_body` split;
  - removing privileged reads;
  - the mother-follow extraction;
  - straying;
  - training code.
  Leave hooks where they obviously belong, and no more.
- **`crates/cubarium-search/src/es/voxel/`** is the TWO-LAYER worker's. Touch it only where
  the trait change forces a compile fix there, and keep that fix minimal.
- The ENGAGE package (Codex) builds a harness in parallel and may add a tiny read-only
  accessor to the fauna crate.

## Checks (written into tests as you go)

- §11.1's checks:
  - slot permutation doesn't change the subject;
  - two targets crossing;
  - a subject dying or vanishing ends the hold;
  - two calves bleating at once.
- A call is heard within range, muffled behind rock, and unheard beyond it. A sleeper
  records a loud call it could hear awake, not a quiet one, and not one behind rock (§7).
- The kick:
  - lands only after windup, on a target in reach and arc;
  - then recovers;
  - a kick-back during recovery is refused, so there's one budget.
- A save and load mid-hold restores the held intent, subject, hold age, recurrent state and
  kick phase.
- Plain manifests are unchanged, and a plain brain's outputs are identical before and after.
- With today's heuristics on a short fixture world (a few hundred ticks), the steering and
  positions match a run with the records stripped. The records ride along.
- Founders: siblings by the shared founding id; no live animal treated as their mother.

## Return

The rules file's format, plus the schema number taken, the interface as built (type names
and file:line), and every place you departed from the doc and why.
